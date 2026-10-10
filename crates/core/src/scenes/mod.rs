pub mod adoption_scene;
pub mod bedroom_scene;
pub mod breakout_scene;
pub mod credits_scene;
pub mod debug_behaviors_scene;
pub mod debug_context_scene;
pub mod debug_env_scene;
pub mod debug_espnow_scene;
pub mod debug_led_scene;
pub mod debug_plants_scene;
pub mod debug_power_scene;
pub mod debug_stats_scene;
pub mod debug_time_scene;
pub mod debug_wifi_scene;
pub mod forecast_scene;
pub mod hanjie_scene;
pub mod herding_scene;
pub mod inside_scene;
pub mod kitchen_scene;
pub mod lightsout_scene;
pub mod maze_scene;
pub mod memory_scene;
pub mod meowltiply_scene;
pub mod menu_scene;
pub mod outside_scene;
pub mod pet_info_scene;
pub mod pipes_scene;
pub mod platformer_scene;
pub mod pose_scene;
pub mod snake_scene;
pub mod social_scene;
pub mod stats_scene;
pub mod store_scene;
pub mod sudokat_scene;
pub mod tictactoe_scene;
pub mod treehouse_scene;
pub mod vacation_aquarium_scene;
pub mod vacation_base;
pub mod vacation_beach_scene;
pub mod vacation_forest_scene;
pub mod vacation_park_scene;
pub mod zoomies_scene;

use crate::scene::{Scene, SceneId};

use adoption_scene::AdoptionScene;
use bedroom_scene::BedroomScene;
use breakout_scene::BreakoutScene;
use credits_scene::CreditsScene;
use debug_behaviors_scene::DebugBehaviorsScene;
use debug_context_scene::DebugContextScene;
use debug_env_scene::DebugEnvScene;
use debug_espnow_scene::DebugEspnowScene;
use debug_led_scene::DebugLedScene;
use debug_plants_scene::DebugPlantsScene;
use debug_power_scene::DebugPowerScene;
use debug_stats_scene::DebugStatsScene;
use debug_time_scene::DebugTimeScene;
use debug_wifi_scene::DebugWifiScene;
use forecast_scene::ForecastScene;
use hanjie_scene::HanjieScene;
use herding_scene::HerdingScene;
use inside_scene::InsideScene;
use kitchen_scene::KitchenScene;
use lightsout_scene::LightsOutScene;
use maze_scene::MazeScene;
use memory_scene::MemoryScene;
use meowltiply_scene::MeowltiplyScene;
use menu_scene::MenuScene;
use outside_scene::OutsideScene;
use pet_info_scene::PetInfoScene;
use pipes_scene::PipesScene;
use platformer_scene::PlatformerScene;
use pose_scene::PoseScene;
use snake_scene::SnakeScene;
use social_scene::SocialScene;
use stats_scene::StatsScene;
use store_scene::StoreScene;
use sudokat_scene::SudokatScene;
use tictactoe_scene::TicTacToeScene;
use treehouse_scene::TreehouseScene;
use vacation_aquarium_scene::VacationAquariumScene;
use vacation_beach_scene::VacationBeachScene;
use vacation_forest_scene::VacationForestScene;
use vacation_park_scene::VacationParkScene;
use zoomies_scene::ZoomiesScene;

crate::dispatch_enum! {
    pub enum ActiveScene from SceneId via from_id,
    as dyn Scene via as_scene / as_scene_mut
    {
        Inside(InsideScene)                       = InsideScene::new(),
        Outside(OutsideScene)                     = OutsideScene::new(),
        Bedroom(BedroomScene)                     = BedroomScene::new(),
        Kitchen(KitchenScene)                     = KitchenScene::new(),
        Treehouse(TreehouseScene)                 = TreehouseScene::new(),
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
        Platformer(PlatformerScene)               = PlatformerScene::new(),
        VacationPark(VacationParkScene)           = VacationParkScene::new(),
        VacationForest(VacationForestScene)       = VacationForestScene::new(),
        VacationAquarium(VacationAquariumScene)   = VacationAquariumScene::new(),
        VacationBeach(VacationBeachScene)         = VacationBeachScene::new(),
    }
}
