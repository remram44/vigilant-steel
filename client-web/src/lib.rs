mod logger;
mod primitives;
mod render;

use game::Game;
use game::input::{Input, Press};
use game::particles::{Effect, EffectInner};
use log::{error, info, warn};
use specs::WorldExt;
use std::cell::{RefCell, RefMut};
use specs::Join;
use wasm_bindgen::prelude::*;

const MAX_TIME_STEP: f32 = 0.040;

#[wasm_bindgen]
extern "C" {
    fn play_sound(sound: i32);
}

pub struct App {
    game: Game,
    render_app: render::RenderApp,
}

static mut _APP: Option<RefCell<App>> = None;

fn get_app<'a>() -> Option<RefMut<'a, App>> {
    match unsafe { &_APP } {
        None => return None,
        Some(ref cell) => Some(cell.borrow_mut()),
    }
}

#[wasm_bindgen(start)]
pub extern "C" fn start() {
    logger::init(log::LevelFilter::Info).unwrap();

    if unsafe { _APP.is_some() } {
        error!("init() called again");
    }
    let app = App {
        game: Game::new_standalone(),
        render_app: Default::default(),
    };
    unsafe {
        _APP = Some(RefCell::new(app));
    }
    info!("initialized");

    render::init();
}

#[wasm_bindgen]
pub extern "C" fn update(
    // Simulation delta
    mut delta: f32,
    // Canvas size
    width: u32, height: u32,
    // Input
    x: f32, y: f32, r: f32, fire: bool,
    mouse_x: f32, mouse_y: f32,
) {
    let mut app = match get_app() {
        None => {
            error!("update() called before init()");
            return;
        }
        Some(a) => a,
    };
    if delta > 0.5 {
        warn!("Clock jumped forward by {} seconds!", delta);
        delta = 5.0 * MAX_TIME_STEP;
    }

    // Set input
    {
        let mut input = app.game.world.write_resource::<Input>();
        input.movement = [x, y];
        input.rotation = r;
        input.fire = if fire { Press::PRESSED } else { Press::UP };
        input.mouse = app.render_app.project_cursor([mouse_x, mouse_y]);
    }

    // Update game simulation
    while delta > 0.0 {
        if delta > MAX_TIME_STEP {
            app.game.update(MAX_TIME_STEP);
            delta -= MAX_TIME_STEP;
        } else {
            app.game.update(delta);
            break;
        }
    }

    // Render
    render::render(&mut app, [width, height]);

    // Play sounds
    let effects = app.game.world.read_component::<Effect>();
    for effect in effects.join() {
        match effect.effect {
            EffectInner::LaserFire => play_sound(1),
            EffectInner::Explosion(_) => {/* TODO */}
            EffectInner::MetalHit => {/* TODO */}
            EffectInner::LaserHit => {/* TODO */}
        }
    }
}
