use bluenoise::WrappingBlueNoise;
use rand::SeedableRng;
use rand_pcg::Pcg64Mcg;

fn main() {
    let mut noise = WrappingBlueNoise::from_rng(16.0, 16.0, 2.0, Pcg64Mcg::seed_from_u64(10));
    let noise = noise.with_samples(10);

    for point in noise.take(10) {
        println!("{}, {}", point.x, point.y);
    }
}
