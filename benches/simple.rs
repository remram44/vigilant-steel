#![feature(test)]

extern crate game;
extern crate test;

use game::Game;
use test::Bencher;

#[bench]
fn flying_asteroids(b: &mut Bencher) {
    b.bytes = 12;
    b.iter(|| {
        let mut game = Game::new_standalone();
        const TIME_STEP: f32 = 0.040;
        // Simulate 20 seconds of game time
        for frame in 0..500 {
            game.update(TIME_STEP);
        }
    });
}
