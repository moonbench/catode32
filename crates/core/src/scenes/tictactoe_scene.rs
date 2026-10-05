//! TicTacToe minigame. Play against the pet on a 3x3, 4x4, or 5x5 board.

use embedded_graphics::prelude::{Point, Size};
use heapless::String;
#[cfg(not(feature = "desktop"))]
use micromath::F32Ext;
use crate::t;

use crate::{
    assets::{
        character::PoseId,
        minigame_assets::{PAW_LARGE1, PAW_MED, PAW_SMALL1},
    },
    context::{GameContext, StatId},
    entities::character::Character,
    input::{Button, Buttons},
    rand::rand_range_u32,
    render::{Renderer, Sprite, SpriteOpts},
    scene::{Scene, SceneId},
    ui::popup::Popup,
};

const BOARD_OFFSET_X: i32 = 2;
const BOARD_OFFSET_Y: i32 = 3;

const MAX_BOARD: usize = 25;
const MAX_LINES: usize = 12;
const MAX_LINE_LEN: usize = 5;

// Cell values.
const EMPTY: u8 = 0;
const PLAYER: u8 = 1;
const PET: u8 = 2;

const PET_THINK_DELAY: f32 = 0.5;
const END_DELAY: f32 = 0.5;

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    PlayerTurn,
    PetTurn,
    PlayerWin,
    PetWin,
    Draw,
}

pub struct TicTacToeScene {
    character: Character,
    result_popup: Popup,

    round_number: u32,
    player_score: f32,
    pet_score: f32,

    board_size: usize,
    cell_size: i32,
    board: [u8; MAX_BOARD],
    win_lines: [[u8; MAX_LINE_LEN]; MAX_LINES],
    win_line_count: usize,

    cursor_pos: usize,
    winning_line: Option<usize>,
    draw_winner: u8,
    pet_think_timer: f32,
    end_delay_timer: f32,
    state: State,
}

impl TicTacToeScene {
    pub fn new() -> Self {
        let mut scene = Self {
            character: Character::new(Point::new(100, 63)),
            result_popup: Popup::new(14, 0, 100, 40),

            round_number: 0,
            player_score: 0.0,
            pet_score: 0.0,

            board_size: 3,
            cell_size: 19,
            board: [EMPTY; MAX_BOARD],
            win_lines: [[0; MAX_LINE_LEN]; MAX_LINES],
            win_line_count: 0,

            cursor_pos: 4,
            winning_line: None,
            draw_winner: EMPTY,
            pet_think_timer: 0.0,
            end_delay_timer: 0.0,
            state: State::PlayerTurn,
        };
        scene.reset_game();
        scene
    }

    fn reset_game(&mut self) {
        self.board_size = if self.round_number < 3 {
            3
        } else if self.round_number < 7 {
            4
        } else {
            5
        };
        self.cell_size = match self.board_size {
            3 => 19,
            4 => 14,
            _ => 11,
        };

        self.generate_win_lines();

        let n = self.board_size;
        for i in 0..MAX_BOARD {
            self.board[i] = EMPTY;
        }
        self.cursor_pos = (n / 2) * n + (n / 2);
        self.winning_line = None;
        self.draw_winner = EMPTY;
        self.pet_think_timer = 0.0;
        self.end_delay_timer = 0.0;

        self.character.set_pose(PoseId::LayingSideNeutral);

        self.state = if self.round_number % 2 == 0 {
            State::PlayerTurn
        } else {
            State::PetTurn
        };
    }

    fn generate_win_lines(&mut self) {
        let n = self.board_size;
        let mut idx = 0;
        // Rows.
        for r in 0..n {
            for c in 0..n {
                self.win_lines[idx][c] = (r * n + c) as u8;
            }
            idx += 1;
        }
        // Columns.
        for c in 0..n {
            for r in 0..n {
                self.win_lines[idx][r] = (r * n + c) as u8;
            }
            idx += 1;
        }
        // Main diagonal.
        for i in 0..n {
            self.win_lines[idx][i] = (i * n + i) as u8;
        }
        idx += 1;
        // Anti-diagonal.
        for i in 0..n {
            self.win_lines[idx][i] = (i * n + (n - 1 - i)) as u8;
        }
        idx += 1;
        self.win_line_count = idx;
    }

    fn line(&self, i: usize) -> &[u8] {
        &self.win_lines[i][..self.board_size]
    }

    fn cell_to_pixel(&self, cell_idx: usize) -> (i32, i32) {
        let row = (cell_idx / self.board_size) as i32;
        let col = (cell_idx % self.board_size) as i32;
        let x = BOARD_OFFSET_X + col * (self.cell_size + 1);
        let y = BOARD_OFFSET_Y + row * (self.cell_size + 1);
        (x, y)
    }

    fn check_winner(&self, mark: u8) -> Option<usize> {
        for li in 0..self.win_line_count {
            let mut won = true;
            for &c in self.line(li) {
                if self.board[c as usize] != mark {
                    won = false;
                    break;
                }
            }
            if won {
                return Some(li);
            }
        }
        None
    }

    fn is_board_full(&self) -> bool {
        let total = self.board_size * self.board_size;
        for i in 0..total {
            if self.board[i] == EMPTY {
                return false;
            }
        }
        true
    }

    fn find_longest_run(&self, mark: u8) -> u8 {
        let mut longest = 0u8;
        for li in 0..self.win_line_count {
            let mut run = 0u8;
            for &c in self.line(li) {
                if self.board[c as usize] == mark {
                    run += 1;
                    if run > longest {
                        longest = run;
                    }
                } else {
                    run = 0;
                }
            }
        }
        longest
    }

    fn resolve_draw(&mut self) {
        self.state = State::Draw;
        self.end_delay_timer = 0.0;
        self.draw_winner = EMPTY;
        if self.board_size > 3 {
            let player_run = self.find_longest_run(PLAYER);
            let pet_run = self.find_longest_run(PET);
            if player_run > pet_run {
                self.draw_winner = PLAYER;
                self.player_score += 0.5;
                self.character.set_pose(PoseId::LayingSideAnnoyed);
            } else if pet_run > player_run {
                self.draw_winner = PET;
                self.pet_score += 0.5;
                self.character.set_pose(PoseId::LayingSideHappy);
            }
        }
        self.set_end_popup();
    }

    fn set_end_popup(&mut self) {
        let mut msg: heapless::String<64> = heapless::String::new();
        match self.state {
            State::PlayerWin => {
                let _ = msg.push_str(t!("You Win!"));
                let _ = msg.push('\n');
                let _ = msg.push_str(t!("A: New Game"));
            }
            State::PetWin => {
                let _ = msg.push_str(t!("You Lose!"));
                let _ = msg.push('\n');
                let _ = msg.push_str(t!("A: New Game"));
            }
            State::Draw => {
                let _ = msg.push_str(t!("Draw!"));
                let _ = msg.push('\n');
                match self.draw_winner {
                    PLAYER => {
                        let _ = msg.push_str(t!("You had the\nlongest row"));
                        let _ = msg.push('\n');
                    }
                    PET => {
                        let _ = msg.push_str(t!("Pet had the\nlongest row"));
                        let _ = msg.push('\n');
                    }
                    _ => {}
                }
                let _ = msg.push_str(t!("A: New Game"));
            }
            _ => return,
        }
        self.result_popup.set_text(msg.as_str(), false, true);
    }

    fn score_cell(&self, cell: usize) -> i32 {
        let n = self.board_size as i32;
        let row = (cell as i32) / n;
        let col = (cell as i32) % n;
        let center = n / 2;
        // Center preference.
        let mut score = n - (row - center).abs() - (col - center).abs();

        let cell_u8 = cell as u8;
        for li in 0..self.win_line_count {
            let line = self.line(li);
            if !line.contains(&cell_u8) {
                continue;
            }
            let mut pet_count = 0i32;
            let mut player_count = 0i32;
            for &c in line {
                match self.board[c as usize] {
                    PET => pet_count += 1,
                    PLAYER => player_count += 1,
                    _ => {}
                }
            }
            if player_count == 0 {
                score += pet_count * 3;
            } else if pet_count == 0 {
                score += player_count;
            }
        }
        score
    }

    fn find_best_move(&mut self, rng: &mut u32) -> Option<usize> {
        let total = self.board_size * self.board_size;
        let mut empty_buf = [0usize; MAX_BOARD];
        let mut empty_len = 0usize;
        for i in 0..total {
            if self.board[i] == EMPTY {
                empty_buf[empty_len] = i;
                empty_len += 1;
            }
        }
        if empty_len == 0 {
            return None;
        }

        // 5% random move (keeps game winnable).
        if rand_range_u32(rng, 1, 100) <= 5 && empty_len > 1 {
            let pick = rand_range_u32(rng, 0, (empty_len - 1) as u32) as usize;
            return Some(empty_buf[pick]);
        }

        // Win immediately.
        for k in 0..empty_len {
            let i = empty_buf[k];
            self.board[i] = PET;
            let won = self.check_winner(PET).is_some();
            self.board[i] = EMPTY;
            if won {
                return Some(i);
            }
        }

        // Block player win.
        for k in 0..empty_len {
            let i = empty_buf[k];
            self.board[i] = PLAYER;
            let won = self.check_winner(PLAYER).is_some();
            self.board[i] = EMPTY;
            if won {
                return Some(i);
            }
        }

        // Score each empty cell.
        let mut best_score = i32::MIN;
        let mut best_buf = [0usize; MAX_BOARD];
        let mut best_len = 0usize;
        for k in 0..empty_len {
            let i = empty_buf[k];
            let s = self.score_cell(i);
            if s > best_score {
                best_score = s;
                best_buf[0] = i;
                best_len = 1;
            } else if s == best_score {
                best_buf[best_len] = i;
                best_len += 1;
            }
        }
        let pick = rand_range_u32(rng, 0, (best_len - 1) as u32) as usize;
        Some(best_buf[pick])
    }

    fn make_pet_move(&mut self, rng: &mut u32) {
        let mut best_move = self.find_best_move(rng);
        if best_move.is_none() {
            let total = self.board_size * self.board_size;
            for i in 0..total {
                if self.board[i] == EMPTY {
                    best_move = Some(i);
                    break;
                }
            }
        }

        if let Some(m) = best_move {
            self.board[m] = PET;
            if let Some(li) = self.check_winner(PET) {
                self.winning_line = Some(li);
                self.state = State::PetWin;
                self.pet_score += 1.0;
                self.end_delay_timer = 0.0;
                self.character.set_pose(PoseId::LayingSideHappy);
                self.set_end_popup();
            } else if self.is_board_full() {
                self.resolve_draw();
            } else {
                self.state = State::PlayerTurn;
            }
        } else {
            self.state = State::PlayerTurn;
        }
    }

    fn player_place_mark(&mut self) {
        if self.board[self.cursor_pos] != EMPTY {
            return;
        }
        self.board[self.cursor_pos] = PLAYER;
        if let Some(li) = self.check_winner(PLAYER) {
            self.winning_line = Some(li);
            self.state = State::PlayerWin;
            self.player_score += 1.0;
            self.end_delay_timer = 0.0;
            self.character.set_pose(PoseId::LayingSideAnnoyed);
            self.set_end_popup();
        } else if self.is_board_full() {
            self.resolve_draw();
        } else {
            self.state = State::PetTurn;
            self.pet_think_timer = 0.0;
        }
    }

    fn draw_board(&self, r: &mut Renderer) {
        let n = self.board_size as i32;
        let board_span = n * self.cell_size + (n - 1);
        for i in 1..n {
            let x = BOARD_OFFSET_X + i * (self.cell_size + 1) - 1;
            r.draw_line(
                Point::new(x, BOARD_OFFSET_Y),
                Point::new(x, BOARD_OFFSET_Y + board_span - 1),
            );
        }
        for i in 1..n {
            let y = BOARD_OFFSET_Y + i * (self.cell_size + 1) - 1;
            r.draw_line(
                Point::new(BOARD_OFFSET_X, y),
                Point::new(BOARD_OFFSET_X + board_span - 1, y),
            );
        }
    }

    fn draw_marks(&self, r: &mut Renderer) {
        let n = self.board_size;
        let (paw, radius) = match n {
            3 => (&PAW_LARGE1 as &Sprite, 7i32),
            4 => (&PAW_MED as &Sprite, 5i32),
            _ => (&PAW_SMALL1 as &Sprite, 4i32),
        };
        let half = self.cell_size / 2;

        for i in 0..n * n {
            if self.board[i] == EMPTY {
                continue;
            }
            let (cx, cy) = self.cell_to_pixel(i);
            if self.board[i] == PLAYER {
                r.draw_circle_outline(Point::new(cx + half, cy + half), radius);
            } else {
                let sx = cx + (self.cell_size - paw.width as i32) / 2;
                let sy = cy + (self.cell_size - paw.height as i32) / 2;
                r.draw_sprite(
                    paw,
                    Point::new(sx, sy),
                    SpriteOpts { transparent: true, ..Default::default() },
                );
            }
        }
    }

    fn draw_cursor(&self, r: &mut Renderer) {
        let (cx, cy) = self.cell_to_pixel(self.cursor_pos);
        let s = (self.cell_size - 2) as u32;
        r.draw_rect(Point::new(cx + 1, cy + 1), Size::new(s, s), false);
    }

    fn draw_score(&self, r: &mut Renderer) {
        let score_x = 62;
        let mut buf: String<8> = String::new();
        r.draw_text(t!("You"), Point::new(score_x, 4));
        format_score(&mut buf, self.player_score);
        r.draw_text(buf.as_str(), Point::new(score_x, 12));
        r.draw_text(t!("Pet"), Point::new(score_x + 28, 4));
        buf.clear();
        format_score(&mut buf, self.pet_score);
        r.draw_text(buf.as_str(), Point::new(score_x + 28, 12));
    }

    fn draw_state_message(&self, r: &mut Renderer) {
        match self.state {
            State::PlayerWin | State::PetWin | State::Draw => {
                self.result_popup.draw(r, false);
            }
            State::PetTurn => {
                r.draw_text("...", Point::new(76, 20));
            }
            _ => {}
        }
    }
}

fn format_score(out: &mut String<8>, v: f32) {
    use core::fmt::Write as _;
    let whole = v as i32;
    if (v - whole as f32).abs() < 0.001 {
        let _ = write!(out, "{}", whole);
    } else {
        let _ = write!(out, "{}.5", whole);
    }
}

impl Scene for TicTacToeScene {
    fn enter(&mut self, _ctx: &mut GameContext) {
        self.round_number = 0;
        self.player_score = 0.0;
        self.pet_score = 0.0;
        self.reset_game();
    }

    fn exit(&mut self, ctx: &mut GameContext) {
        let current_ended = matches!(
            self.state,
            State::PlayerWin | State::PetWin | State::Draw
        );
        let total_rounds = self.round_number + if current_ended { 1 } else { 0 };
        if total_rounds == 0 {
            return;
        }
        let scale = (total_rounds as f32 / 8.0).sqrt();
        ctx.apply_stat_changes(&[
            (StatId::Sociability,  3.0 * scale),
            (StatId::Intelligence, 4.0 * scale),
            (StatId::Focus,        3.0 * scale),
            (StatId::Fulfillment,  3.0 * scale),
            (StatId::Loyalty,      1.0 * scale),
        ]);
        let coins = 2 * total_rounds as i32;
        if coins > 0 {
            ctx.coins += coins;
        }
    }

    fn update(
        &mut self,
        ctx: &mut GameContext,
        buttons: &mut Buttons,
        dt: f32,
    ) -> Option<SceneId> {
        if buttons.was_just_pressed(Button::Menu1) {
            return Some(SceneId::Menu);
        }

        self.character.animate(dt);

        if self.state == State::PetTurn {
            self.pet_think_timer += dt;
            if self.pet_think_timer >= PET_THINK_DELAY {
                let mut rng = ctx.rng;
                self.make_pet_move(&mut rng);
                ctx.rng = rng;
                self.pet_think_timer = 0.0;
            }
        }

        if matches!(
            self.state,
            State::PlayerWin | State::PetWin | State::Draw
        ) {
            self.end_delay_timer += dt;
            if buttons.was_just_pressed(Button::A) && self.end_delay_timer >= END_DELAY {
                self.round_number += 1;
                self.reset_game();
            }
            return None;
        }

        if self.state == State::PlayerTurn {
            let n = self.board_size;
            let row = self.cursor_pos / n;
            let col = self.cursor_pos % n;

            if buttons.was_just_pressed(Button::Up) && row > 0 {
                self.cursor_pos -= n;
            } else if buttons.was_just_pressed(Button::Down) && row < n - 1 {
                self.cursor_pos += n;
            } else if buttons.was_just_pressed(Button::Left) && col > 0 {
                self.cursor_pos -= 1;
            } else if buttons.was_just_pressed(Button::Right) && col < n - 1 {
                self.cursor_pos += 1;
            }

            if buttons.was_just_pressed(Button::A) {
                self.player_place_mark();
            }
        }

        None
    }

    fn draw(&self, _ctx: &GameContext, renderer: &mut Renderer, _dt_ms: u64) {
        self.draw_board(renderer);
        self.draw_marks(renderer);
        if self.state == State::PlayerTurn {
            self.draw_cursor(renderer);
        }
        self.draw_score(renderer);
        self.draw_state_message(renderer);
        self.character.draw(renderer, 0);
    }
}
