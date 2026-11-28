pub mod adoption_scene;
pub mod balancing_scene;
pub mod bedroom_scene;
pub mod boxedin_scene;
pub mod breakout_scene;
pub mod breathe_scene;
pub mod calculator_scene;
pub mod cattree_scene;
pub mod credits_scene;
pub mod debug_behaviors_scene;
pub mod debug_context_scene;
pub mod debug_env_scene;
pub mod debug_espnow_scene;
#[cfg(feature = "rgb-led")]
pub mod debug_led_scene;
pub mod debug_plants_scene;
pub mod debug_power_scene;
pub mod debug_stats_scene;
pub mod debug_time_scene;
pub mod debug_wifi_scene;
pub mod dice_scene;
pub mod eightball_scene;
#[cfg(feature = "rgb-led")]
pub mod flashlight_scene;
pub mod forecast_scene;
pub mod garage_scene;
pub mod hanjie_scene;
pub mod herding_scene;
pub mod hideseek_scene;
pub mod inside_scene;
pub mod kitchen_scene;
pub mod laserbounce_scene;
pub mod life_scene;
pub mod lightsout_scene;
pub mod lucky_scene;
pub mod luckylanding_scene;
pub mod maze_scene;
pub mod memory_scene;
pub mod menu_scene;
pub mod meowltiply_scene;
pub mod meowteor_scene;
pub mod metronome_scene;
pub mod minigolf_scene;
pub mod morse_scene;
pub mod outside_scene;
pub mod pawpong_scene;
pub mod pestcontrol_scene;
pub mod pet_info_scene;
pub mod pipes_scene;
pub mod platformer_scene;
pub mod pomodoro_scene;
pub mod pose_scene;
pub mod purrimeter_scene;
pub mod remote_scene;
pub mod reversi_scene;
pub mod save_recovery_scene;
pub mod scratch_scene;
pub mod sleepingdogs_scene;
pub mod snake_scene;
pub mod social_scene;
pub mod stats_scene;
pub mod store_scene;
pub mod sudokat_scene;
pub mod sunnyspot_scene;
pub mod tictactoe_scene;
pub mod timer_scene;
pub mod treattoss_scene;
pub mod treehouse_scene;
pub mod vacation_aquarium_scene;
pub mod vacation_base;
pub mod vacation_beach_scene;
pub mod vacation_forest_scene;
pub mod vacation_park_scene;
pub mod wifiscan_scene;
pub mod yarnball_scene;
pub mod zoomies_scene;

use crate::scene::{Scene, SceneId};

use adoption_scene::AdoptionScene;
use bedroom_scene::BedroomScene;
use balancing_scene::BalancingScene;
use boxedin_scene::BoxedInScene;
use breakout_scene::BreakoutScene;
use cattree_scene::CatTreeScene;
use credits_scene::CreditsScene;
use debug_behaviors_scene::DebugBehaviorsScene;
use debug_context_scene::DebugContextScene;
use debug_env_scene::DebugEnvScene;
use debug_espnow_scene::DebugEspnowScene;
#[cfg(feature = "rgb-led")]
use debug_led_scene::DebugLedScene;
use debug_plants_scene::DebugPlantsScene;
use debug_power_scene::DebugPowerScene;
use debug_stats_scene::DebugStatsScene;
use debug_time_scene::DebugTimeScene;
use debug_wifi_scene::DebugWifiScene;
use forecast_scene::ForecastScene;
use garage_scene::GarageScene;
use save_recovery_scene::SaveRecoveryScene;
use hanjie_scene::HanjieScene;
use herding_scene::HerdingScene;
use hideseek_scene::HideSeekScene;
use inside_scene::InsideScene;
use kitchen_scene::KitchenScene;
use life_scene::LifeScene;
use lightsout_scene::LightsOutScene;
use luckylanding_scene::LuckyLandingScene;
use maze_scene::MazeScene;
use memory_scene::MemoryScene;
use meowltiply_scene::MeowltiplyScene;
use menu_scene::MenuScene;
use outside_scene::OutsideScene;
use minigolf_scene::MiniGolfScene;
use laserbounce_scene::LaserBounceScene;
use pawpong_scene::PawPongScene;
use pestcontrol_scene::PestControlScene;
use meowteor_scene::MeowteorScene;
use scratch_scene::ScratchScene;
use pet_info_scene::PetInfoScene;
use pipes_scene::PipesScene;
use platformer_scene::PlatformerScene;
use pose_scene::PoseScene;
use purrimeter_scene::PurrimeterScene;
use reversi_scene::ReversiScene;
use sleepingdogs_scene::SleepingDogsScene;
use snake_scene::SnakeScene;
use social_scene::SocialScene;
use stats_scene::StatsScene;
use store_scene::StoreScene;
use sudokat_scene::SudokatScene;
use sunnyspot_scene::SunnySpotScene;
use tictactoe_scene::TicTacToeScene;
use treattoss_scene::TreatTossScene;
use treehouse_scene::TreehouseScene;
use vacation_aquarium_scene::VacationAquariumScene;
use vacation_beach_scene::VacationBeachScene;
use vacation_forest_scene::VacationForestScene;
use vacation_park_scene::VacationParkScene;
use yarnball_scene::YarnballScene;
use zoomies_scene::ZoomiesScene;
use dice_scene::DiceScene;
use lucky_scene::LuckyScene;
#[cfg(feature = "rgb-led")]
use flashlight_scene::FlashlightScene;
use eightball_scene::EightBallScene;
use calculator_scene::CalculatorScene;
use wifiscan_scene::WifiScanScene;
use timer_scene::TimerScene;
use pomodoro_scene::PomodoroScene;
use breathe_scene::BreatheScene;
use metronome_scene::MetronomeScene;
use morse_scene::MorseScene;
use remote_scene::RemoteScene;

crate::dispatch_enum! {
    pub enum ActiveScene from SceneId via from_id,
    as dyn Scene via as_scene / as_scene_mut
    {
        Inside(InsideScene)                       = InsideScene::new(),
        Outside(OutsideScene)                     = OutsideScene::new(),
        Bedroom(BedroomScene)                     = BedroomScene::new(),
        Kitchen(KitchenScene)                     = KitchenScene::new(),
        Treehouse(TreehouseScene)                 = TreehouseScene::new(),
        Garage(GarageScene)                       = GarageScene::new(),
        SaveRecovery(SaveRecoveryScene)           = SaveRecoveryScene::new(),
        Menu(MenuScene)                           = MenuScene::new(),
        PoseViewer(PoseScene)                     = PoseScene::new(),
        Stats(StatsScene)                         = StatsScene::new(),
        Forecast(ForecastScene)                   = ForecastScene::new(),
        Store(StoreScene)                         = StoreScene::new(),
        Adoption(AdoptionScene)                   = AdoptionScene::new(),
        PetInfo(PetInfoScene)                     = PetInfoScene::new(),
        Credits(CreditsScene)                     = CreditsScene::new(),
        DebugBehaviors(DebugBehaviorsScene)       = DebugBehaviorsScene::new(),
        DebugContext(DebugContextScene)           = DebugContextScene::new(),
        DebugEnv(DebugEnvScene)                   = DebugEnvScene::new(),
        #[cfg(feature = "rgb-led")]
        DebugLed(DebugLedScene)                   = DebugLedScene::new(),
        DebugPlants(DebugPlantsScene)             = DebugPlantsScene::new(),
        DebugPower(DebugPowerScene)               = DebugPowerScene::new(),
        DebugStats(DebugStatsScene)               = DebugStatsScene::new(),
        DebugTime(DebugTimeScene)                 = DebugTimeScene::new(),
        DebugWifi(DebugWifiScene)                 = DebugWifiScene::new(),
        DebugEspnow(DebugEspnowScene)             = DebugEspnowScene::new(),
        Social(SocialScene)                       = SocialScene::new(),
        Zoomies(ZoomiesScene)                     = ZoomiesScene::new(),
        Breakout(BreakoutScene)                   = BreakoutScene::new(),
        Snake(SnakeScene)                         = SnakeScene::new(),
        Memory(MemoryScene)                       = MemoryScene::new(),
        Maze(MazeScene)                           = MazeScene::new(),
        Hanjie(HanjieScene)                       = HanjieScene::new(),
        TicTacToe(TicTacToeScene)                 = TicTacToeScene::new(),
        LightsOut(LightsOutScene)                 = LightsOutScene::new(),
        Pipes(PipesScene)                         = PipesScene::new(),
        Herding(HerdingScene)                     = HerdingScene::new(),
        Meowltiply(MeowltiplyScene)               = MeowltiplyScene::new(),
        Sudokat(SudokatScene)                     = SudokatScene::new(),
        Reversi(ReversiScene)                     = ReversiScene::new(),
        HideSeek(HideSeekScene)                   = HideSeekScene::new(),
        TreatToss(TreatTossScene)                 = TreatTossScene::new(),
        SunnySpot(SunnySpotScene)                 = SunnySpotScene::new(),
        Purrimeter(PurrimeterScene)               = PurrimeterScene::new(),
        LuckyLanding(LuckyLandingScene)           = LuckyLandingScene::new(),
        Yarnball(YarnballScene)                   = YarnballScene::new(),
        BalancingAct(BalancingScene)              = BalancingScene::new(),
        CatTree(CatTreeScene)                     = CatTreeScene::new(),
        SleepingDogs(SleepingDogsScene)           = SleepingDogsScene::new(),
        MiniGolf(MiniGolfScene)                   = MiniGolfScene::new(),
        LaserBounce(LaserBounceScene)             = LaserBounceScene::new(),
        BoxedIn(BoxedInScene)                     = BoxedInScene::new(),
        PestControl(PestControlScene)             = PestControlScene::new(),
        PawPong(PawPongScene)                     = PawPongScene::new(),
        Life(LifeScene)                           = LifeScene::new(),
        Meowteor(MeowteorScene)                   = MeowteorScene::new(),
        Scratch(ScratchScene)                     = ScratchScene::new(),
        Platformer(PlatformerScene)               = PlatformerScene::new(),
        VacationPark(VacationParkScene)           = VacationParkScene::new(),
        VacationForest(VacationForestScene)       = VacationForestScene::new(),
        VacationAquarium(VacationAquariumScene)   = VacationAquariumScene::new(),
        VacationBeach(VacationBeachScene)         = VacationBeachScene::new(),
        Dice(DiceScene)                           = DiceScene::new(),
        LuckyNumbers(LuckyScene)                  = LuckyScene::new(),
        #[cfg(feature = "rgb-led")]
        Flashlight(FlashlightScene)               = FlashlightScene::new(),
        EightBall(EightBallScene)                 = EightBallScene::new(),
        Calculator(CalculatorScene)               = CalculatorScene::new(),
        WifiScanner(WifiScanScene)                = WifiScanScene::new(),
        Timer(TimerScene)                         = TimerScene::new(),
        Pomodoro(PomodoroScene)                   = PomodoroScene::new(),
        Breathe(BreatheScene)                     = BreatheScene::new(),
        Metronome(MetronomeScene)                 = MetronomeScene::new(),
        Morse(MorseScene)                         = MorseScene::new(),
        Remote(RemoteScene)                       = RemoteScene::new(),
    }
}
