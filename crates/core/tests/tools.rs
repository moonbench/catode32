//! Mechanics of the Tools menu: the calculator engine, lucky numbers and
//! dice rolls, plus enter/exit of every tool scene.

#![cfg(feature = "desktop")]

use catode32_core::{
    calc::{evaluate, format_number, CalcError, Calculator, Key},
    context::{FlashlightPrefs, GameContext},
    input::{ButtonPin, Buttons},
    led::Led,
    render::Renderer,
    scene::Scene,
    scenes::{
        calculator_scene::CalculatorScene,
        breathe_scene::{fullness, step_at, BreatheScene, PATTERNS},
        dice_scene::{flip, pick_in_range, roll, step_range_value, total, DiceScene, RANGE_MAX, SIDES},
        eightball_scene::EightBallScene,
        flashlight_scene::FlashlightScene,
        lucky_scene::{lucky_for, LuckyScene, FORTUNES, LUCKY_COUNT, LUCKY_MAX},
        metronome_scene::{beat_period_us, tapped_bpm, MetronomeScene, BPM_MAX, BPM_MIN},
        morse_scene::{classify, decode, encode, unit_ms, MorseScene, CODES},
        pomodoro_scene::{next_phase, Phase, PomodoroScene},
        timer_scene::{interval_at, TimerScene},
        wifiscan_scene::{SignalTrack, WifiScanScene, TRACK_HISTORY},
    },
};

fn eval(s: &str) -> f64 {
    evaluate(s).unwrap_or_else(|e| panic!("{s:?} failed: {e:?}"))
}

fn shown(v: f64) -> String {
    format_number(v).as_str().to_string()
}

/// Type `keys` on a fresh calculator. Digits, `.+-*/^()` map to their
/// keys, `=` evaluates, `C` clears, `<` is backspace.
fn type_keys(keys: &str) -> Calculator {
    let mut c = Calculator::new();
    for ch in keys.chars() {
        match ch {
            '0'..='9' => c.press(Key::Digit(ch as u8 - b'0')),
            '.' => c.press(Key::Dot),
            '+' => c.press(Key::Add),
            '-' => c.press(Key::Sub),
            '*' => c.press(Key::Mul),
            '/' => c.press(Key::Div),
            '^' => c.press(Key::Pow),
            '(' => c.press(Key::Open),
            ')' => c.press(Key::Close),
            '=' => c.press(Key::Equals),
            'C' => c.press(Key::Clear),
            '<' => c.backspace(),
            _ => panic!("no key for {ch:?}"),
        }
    }
    c
}

// --- Calculator: parsing and arithmetic ------------------------------------

#[test]
fn calc_precedence() {
    assert_eq!(eval("2+3*4"), 14.0);
    assert_eq!(eval("2*3+4"), 10.0);
    assert_eq!(eval("10-4-3"), 3.0);
    assert_eq!(eval("100/10/5"), 2.0);
}

#[test]
fn calc_parentheses() {
    assert_eq!(eval("(2+3)*4"), 20.0);
    assert_eq!(eval("((1+2)*(3+4))"), 21.0);
    assert_eq!(eval("2*(3+(4-1))"), 12.0);
    // Unclosed parentheses close at the end.
    assert_eq!(eval("(2+3"), 5.0);
    assert_eq!(eval("2*(1+(2"), 6.0);
}

#[test]
fn calc_exponents() {
    assert_eq!(eval("2^10"), 1024.0);
    assert_eq!(eval("2^3^2"), 512.0, "right-associative");
    assert_eq!(eval("-2^2"), -4.0, "unary minus binds looser than ^");
    assert_eq!(eval("(-2)^2"), 4.0);
    assert_eq!(eval("(-2)^3"), -8.0);
    assert_eq!(eval("2^-1"), 0.5);
    assert_eq!(eval("2*3^2"), 18.0);
    assert_eq!(eval("5^0"), 1.0);
    assert_eq!(shown(eval("2^0.5")), "1.4142135624");
    assert_eq!(shown(eval("9^0.5")), "3");
    assert_eq!(shown(eval("1000^(1/3)")), "10");
    assert_eq!(shown(eval("2.5^1.5")), "3.9528470752");
}

#[test]
fn calc_implicit_multiplication() {
    assert_eq!(eval("2(3)"), 6.0);
    assert_eq!(eval("(1+2)(3+4)"), 21.0);
    assert_eq!(eval("(2)3"), 6.0);
}

#[test]
fn calc_decimals() {
    assert_eq!(shown(eval("0.1+0.2")), "0.3");
    assert_eq!(shown(eval("1/3")), "0.3333333333");
    assert_eq!(shown(eval(".5*4")), "2");
    assert_eq!(shown(eval("2/3")), "0.6666666667");
}

#[test]
fn calc_errors() {
    assert_eq!(evaluate("1/0"), Err(CalcError::DivideByZero));
    assert_eq!(evaluate("0^-1"), Err(CalcError::DivideByZero));
    assert_eq!(evaluate("(-8)^0.5"), Err(CalcError::Domain));
    assert_eq!(evaluate("2+"), Err(CalcError::Syntax));
    assert_eq!(evaluate("*2"), Err(CalcError::Syntax));
    assert_eq!(evaluate("2)"), Err(CalcError::Syntax));
    assert_eq!(evaluate(""), Err(CalcError::Syntax));
    assert_eq!(evaluate("10^400"), Err(CalcError::Overflow));
    let deep = format!("{}1", "(".repeat(40));
    assert_eq!(evaluate(&deep), Err(CalcError::TooDeep));
}

#[test]
fn calc_formatting() {
    assert_eq!(shown(0.0), "0");
    assert_eq!(shown(-0.0), "0");
    assert_eq!(shown(42.0), "42");
    assert_eq!(shown(-1.5), "-1.5");
    assert_eq!(shown(12345678901.0), "12345678901");
    assert_eq!(shown(123456789012.0), "123456789012");
    assert_eq!(shown(1234567890123.0), "1.2345679e12");
    assert_eq!(shown(1e15), "1e15");
    assert_eq!(shown(-1.234567e20), "-1.234567e20");
    assert_eq!(shown(0.000123), "0.000123");
    assert_eq!(shown(1.5e-12), "1.5e-12");
    for v in [1e300, -1e-300, 123456.789, 2.0f64.sqrt() * 1e9, -987654321.123] {
        assert!(shown(v).len() <= 12, "{v} -> {}", shown(v));
    }
    // A formatted answer parses back to (nearly) the same value.
    for v in [1e15, -2.5e-7, 3.14159] {
        let back = eval(&shown(v));
        assert!((back - v).abs() <= v.abs() * 1e-9, "{v} -> {back}");
    }
}

// --- Calculator: typing ------------------------------------------------------

#[test]
fn typing_builds_the_expression() {
    let c = type_keys("12+3*(4-1)");
    assert_eq!(c.expr(), "12+3*(4-1)");
    assert_eq!(c.result_text(), "21", "live preview");
    assert!(!c.is_evaluated());

    let c = type_keys("12+3*(4-1)=");
    assert!(c.is_evaluated());
    assert_eq!(c.result_text(), "21");
}

#[test]
fn typing_guards_against_junk() {
    assert_eq!(type_keys("1.2.3").expr(), "1.23", "second dot in a number ignored");
    assert_eq!(type_keys(".5").expr(), "0.5", "leading dot gets a zero");
    assert_eq!(type_keys("2+*3").expr(), "2*3", "operator replaces operator");
    assert_eq!(type_keys("*3").expr(), "3", "binary op needs a left side");
    assert_eq!(type_keys("2)").expr(), "2", "no unmatched close");
    assert_eq!(type_keys("(2+)").expr(), "(2+", "close needs a value before it");
    assert_eq!(type_keys("2^-1").expr(), "2^-1", "unary minus after ^");
    assert_eq!(type_keys("2+-3").expr(), "2-3", "minus replaces plus");
}

#[test]
fn typing_backspace_and_clear() {
    let c = type_keys("123<<4");
    assert_eq!(c.expr(), "14");
    let c = type_keys("12+3=<");
    assert_eq!(c.expr(), "12+", "backspace after = edits the expression");
    assert!(!c.is_evaluated());
    let c = type_keys("12+3C");
    assert_eq!(c.expr(), "");
    assert_eq!(c.result_text(), "");
}

#[test]
fn typing_carries_on_from_the_answer() {
    let c = type_keys("2+3=*4=");
    assert_eq!(c.result_text(), "20");
    let c = type_keys("2+3=7");
    assert_eq!(c.expr(), "7", "a digit after = starts fresh");
    let c = type_keys("1/0=+1");
    assert_eq!(c.expr(), "1", "nothing to carry from an error, so + is dropped");
    let c = type_keys("1/0=");
    assert_eq!(c.result_text(), "Err");
}

// --- Lucky numbers -----------------------------------------------------------

#[test]
fn lucky_numbers_are_stable_for_a_day() {
    assert_eq!(lucky_for(0xABCDEF, 7), lucky_for(0xABCDEF, 7));
}

#[test]
fn lucky_numbers_are_six_distinct_sorted_in_range() {
    for day in 0..200 {
        let (nums, fortune) = lucky_for(0x5EED_0000 + day as u64 * 31, day);
        assert_eq!(nums.len(), LUCKY_COUNT);
        assert!(nums.windows(2).all(|w| w[0] < w[1]), "sorted and distinct: {nums:?}");
        assert!(nums.iter().all(|&n| n <= LUCKY_MAX));
        assert!(fortune < FORTUNES.len());
    }
}

#[test]
fn lucky_numbers_change_with_day_and_pet() {
    let seed = 0x1234_5678;
    let changed_days = (1..50).filter(|&d| lucky_for(seed, d).0 != lucky_for(seed, 0).0).count();
    assert!(changed_days >= 48, "only {changed_days}/49 days differ");
    assert_ne!(lucky_for(1, 3).0, lucky_for(2, 3).0);
}

// --- Dice --------------------------------------------------------------------

#[test]
fn dice_rolls_respect_count_and_sides() {
    let mut rng = 0xDEAD_BEEF;
    for &sides in SIDES.iter() {
        for count in 1..=6 {
            let values = roll(count, sides, &mut rng);
            assert_eq!(values.len(), count);
            assert!(values.iter().all(|&v| v >= 1 && v <= sides), "d{sides}: {values:?}");
            assert_eq!(total(&values), values.iter().map(|&v| v as u32).sum::<u32>());
        }
    }
}

#[test]
fn dice_reach_every_face() {
    let mut rng = 42;
    let mut seen = [false; 21];
    for _ in 0..2000 {
        for v in roll(1, 20, &mut rng) {
            seen[v as usize] = true;
        }
    }
    assert!(seen[1..=20].iter().all(|&s| s), "{seen:?}");
}

// --- Scenes ------------------------------------------------------------------

fn buttons() -> Buttons {
    Buttons::new([0, 1, 2, 3, 4, 5, 6, 7].map(ButtonPin::new))
}

fn exercise(scene: &mut dyn Scene, ctx: &mut GameContext) {
    let mut r = Renderer::new();
    let mut b = buttons();
    scene.enter(ctx);
    for _ in 0..60 {
        assert!(scene.update(ctx, &mut b, 1.0 / 12.0).is_none());
        r.clear();
        scene.draw(ctx, &mut r, 83);
    }
    scene.tick_background(ctx, 0.5);
    scene.exit(ctx);
}

#[test]
fn tool_scenes_enter_run_and_exit() {
    let mut ctx = GameContext::new(Led::new());
    ctx.pet_seed = 99;
    exercise(&mut CalculatorScene::new(), &mut ctx);
    exercise(&mut DiceScene::new(), &mut ctx);
    exercise(&mut LuckyScene::new(), &mut ctx);
    exercise(&mut EightBallScene::new(), &mut ctx);
    exercise(&mut WifiScanScene::new(), &mut ctx);
    exercise(&mut TimerScene::new(), &mut ctx);
    exercise(&mut PomodoroScene::new(), &mut ctx);
    exercise(&mut BreatheScene::new(), &mut ctx);
    exercise(&mut MetronomeScene::new(), &mut ctx);
    exercise(&mut MorseScene::new(), &mut ctx);
    assert!(!ctx.keep_awake, "every tool lets the device sleep again on exit");
}

#[test]
fn flashlight_holds_off_sleep_only_while_open() {
    let mut ctx = GameContext::new(Led::new());
    let mut scene = FlashlightScene::new();
    let mut b = buttons();
    scene.enter(&mut ctx);
    assert!(ctx.keep_awake, "light starts on and keeps the device awake");
    scene.update(&mut ctx, &mut b, 0.1);
    assert!(ctx.keep_awake);
    scene.exit(&mut ctx);
    assert!(!ctx.keep_awake, "leaving releases the hold");
}

#[test]
fn flashlight_goes_dark_while_the_menu_is_open() {
    let mut ctx = GameContext::new(Led::new());
    let mut scene = FlashlightScene::new();
    let mut b = buttons();
    scene.enter(&mut ctx);
    scene.tick_background(&mut ctx, 0.1);
    assert!(!ctx.keep_awake, "menu open: light out, sleep allowed");
    scene.update(&mut ctx, &mut b, 0.1);
    assert!(ctx.keep_awake, "menu closed: light back on");
}

#[test]
fn flashlight_reopens_as_it_was_left() {
    let mut ctx = GameContext::new(Led::new());
    let prefs = FlashlightPrefs { mode: 6, level: 2, rgb: [1, 2, 3] };
    ctx.flashlight = prefs;
    let mut scene = FlashlightScene::new();
    let mut b = buttons();
    scene.enter(&mut ctx);
    scene.update(&mut ctx, &mut b, 0.1);
    scene.exit(&mut ctx);
    assert_eq!(ctx.flashlight, prefs);
}

#[test]
fn flashlight_tolerates_out_of_range_prefs() {
    let mut ctx = GameContext::new(Led::new());
    ctx.flashlight = FlashlightPrefs { mode: 200, level: 200, rgb: [255; 3] };
    let mut scene = FlashlightScene::new();
    let mut r = Renderer::new();
    scene.enter(&mut ctx);
    scene.draw(&ctx, &mut r, 83);
    scene.exit(&mut ctx);
}

// --- Coins and ranges --------------------------------------------------------

#[test]
fn coins_flip_both_ways() {
    let mut rng = 7;
    let flips: Vec<bool> = (0..200).flat_map(|_| flip(1, &mut rng)).collect();
    assert!(flips.iter().any(|&h| h) && flips.iter().any(|&h| !h));
    assert_eq!(flip(4, &mut rng).len(), 4);
}

#[test]
fn range_picks_stay_inside_either_order() {
    let mut rng = 99;
    for _ in 0..500 {
        let v = pick_in_range(5, 9, &mut rng);
        assert!((5..=9).contains(&v));
        let w = pick_in_range(9, 5, &mut rng);
        assert!((5..=9).contains(&w));
    }
    assert_eq!(pick_in_range(42, 42, &mut rng), 42);
}

#[test]
fn range_steps_snap_to_the_step_size() {
    assert_eq!(step_range_value(7, 1, 1), 8);
    assert_eq!(step_range_value(7, 1, 10), 10);
    assert_eq!(step_range_value(17, -1, 10), 10);
    assert_eq!(step_range_value(20, -1, 10), 10);
    assert_eq!(step_range_value(250, 1, 100), 300);
    assert_eq!(step_range_value(0, -1, 1), 0);
    assert_eq!(step_range_value(RANGE_MAX, 1, 100), RANGE_MAX);
}

// --- Timers ------------------------------------------------------------------

#[test]
fn interval_phases_alternate_without_a_final_rest() {
    // 30 s work, 10 s rest, 3 rounds = 30+10+30+10+30 = 110 s.
    assert_eq!(interval_at(30, 10, 3, 0), Some((1, true, 30_000)));
    assert_eq!(interval_at(30, 10, 3, 30_000), Some((1, false, 10_000)));
    assert_eq!(interval_at(30, 10, 3, 40_000), Some((2, true, 30_000)));
    assert_eq!(interval_at(30, 10, 3, 109_999), Some((3, true, 1)));
    assert_eq!(interval_at(30, 10, 3, 110_000), None);
    // No rest at all.
    assert_eq!(interval_at(20, 0, 2, 20_000), Some((2, true, 20_000)));
}

#[test]
fn pomodoro_takes_a_long_break_every_fourth_session() {
    assert_eq!(next_phase(Phase::Focus, 1), Phase::Break);
    assert_eq!(next_phase(Phase::Focus, 3), Phase::Break);
    assert_eq!(next_phase(Phase::Focus, 4), Phase::LongBreak);
    assert_eq!(next_phase(Phase::Focus, 8), Phase::LongBreak);
    assert_eq!(next_phase(Phase::Break, 1), Phase::Focus);
    assert_eq!(next_phase(Phase::LongBreak, 4), Phase::Focus);
}

#[test]
fn breathing_follows_the_pattern() {
    let box_ = &PATTERNS[0]; // 4-4-4-4
    assert_eq!(step_at(box_, 1.0).0, 0);
    assert_eq!(step_at(box_, 5.0).0, 1);
    assert_eq!(step_at(box_, 9.0).0, 2);
    assert_eq!(step_at(box_, 13.0).0, 3);
    assert_eq!(step_at(box_, 17.0).0, 0, "cycles round");
    let even = &PATTERNS[2]; // 5-0-5-0: zero-length holds are skipped
    assert_eq!(step_at(even, 6.0).0, 2);
    assert_eq!(fullness(0, 0.0), 0.0);
    assert_eq!(fullness(0, 1.0), 1.0);
    assert_eq!(fullness(1, 0.5), 1.0);
    assert_eq!(fullness(2, 1.0), 0.0);
    assert!(fullness(0, 0.25) < fullness(0, 0.75));
}

#[test]
fn metronome_timing() {
    assert_eq!(beat_period_us(60), 1_000_000);
    assert_eq!(beat_period_us(120), 500_000);
    assert_eq!(beat_period_us(1), beat_period_us(BPM_MIN));
    assert_eq!(tapped_bpm(&[0, 500_000, 1_000_000, 1_500_000]), Some(120));
    assert_eq!(tapped_bpm(&[0]), None);
    assert_eq!(tapped_bpm(&[0, 10]), Some(BPM_MAX));
}

#[test]
fn led_cues_queue_in_time_order_and_fire_when_due() {
    let mut ctx = GameContext::new(Led::new());
    assert!(ctx.schedule_led(300, (3, 3, 3)));
    assert!(ctx.schedule_led(100, (1, 1, 1)));
    assert!(ctx.schedule_led(200, (2, 2, 2)));
    let times: Vec<u64> = ctx.led_cues.iter().map(|c| c.at_us).collect();
    assert_eq!(times, [100, 200, 300]);
    ctx.apply_due_led_cues(200);
    assert_eq!(ctx.led_cues.len(), 1);
    assert_eq!(ctx.led_cues[0].at_us, 300);
}

// --- Morse -------------------------------------------------------------------

#[test]
fn morse_round_trips_every_character() {
    for &(c, code) in CODES.iter() {
        assert_eq!(encode(c), Some(code));
        assert_eq!(decode(code), Some(c));
    }
    assert_eq!(encode('s'), Some("..."));
    assert_eq!(decode("......."), None);
    let codes: std::collections::HashSet<&str> = CODES.iter().map(|&(_, c)| c).collect();
    assert_eq!(codes.len(), CODES.len(), "codes are unique");
}

#[test]
fn morse_keying_splits_dots_from_dashes() {
    let unit = unit_ms(10);
    assert_eq!(unit, 120);
    assert_eq!(classify(100, unit), '.');
    assert_eq!(classify(239, unit), '.');
    assert_eq!(classify(240, unit), '-');
    assert_eq!(classify(400, unit), '-');
}

// --- WiFi detail graph -----------------------------------------------------

#[test]
fn signal_track_records_misses() {
    let mut t = SignalTrack::new();
    t.push(Some(-70));
    t.push(None);
    assert_eq!(t.latest(), None, "the last scan missed it");
    t.push(Some(-60));
    assert_eq!(t.latest(), Some(-60));
    assert_eq!(t.history().collect::<std::vec::Vec<_>>(), [Some(-70), None, Some(-60)]);
}

#[test]
fn signal_track_history_is_bounded() {
    let mut t = SignalTrack::new();
    for i in 0..(TRACK_HISTORY as i32 + 10) {
        t.push(Some((-90 + (i % 30)) as i8));
    }
    assert_eq!(t.history().count(), TRACK_HISTORY);
    t.reset();
    assert_eq!(t.history().count(), 0);
}
