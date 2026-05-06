use std::time::Instant;

use klattsch_core::FormantSynth;
use klattsch_text::{compile_string, CompileOptions};

fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("usage: bench_render <input.txt> [iterations]");
    let iters: usize = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(100);

    let text = std::fs::read_to_string(&path).expect("read input");
    let bytes = text.len();
    let opts = CompileOptions::default();

    // Warmup
    for _ in 0..5 {
        let r = compile_string(&text, &opts).expect("compile");
        let total = ((r.total_ms * opts.sample_rate as f32 / 1000.0).ceil() as usize).max(1);
        let mut buf = vec![0.0f32; total];
        let mut s = FormantSynth::new(opts.sample_rate);
        s.queue_schedule(r.schedule);
        s.process(&mut buf);
        std::hint::black_box(buf);
    }

    let mut compile_ns: Vec<u128> = Vec::with_capacity(iters);
    let mut render_ns: Vec<u128> = Vec::with_capacity(iters);
    let mut total_samples = 0usize;
    let mut total_ms_sum = 0.0f64;

    for _ in 0..iters {
        let t0 = Instant::now();
        let r = compile_string(&text, &opts).expect("compile");
        let t1 = Instant::now();
        let total = ((r.total_ms * opts.sample_rate as f32 / 1000.0).ceil() as usize).max(1);
        let mut buf = vec![0.0f32; total];
        let mut s = FormantSynth::new(opts.sample_rate);
        s.queue_schedule(r.schedule);
        s.process(&mut buf);
        let t2 = Instant::now();
        std::hint::black_box(&buf);
        compile_ns.push((t1 - t0).as_nanos());
        render_ns.push((t2 - t1).as_nanos());
        total_samples = buf.len();
        total_ms_sum = r.total_ms as f64;
    }

    let stats = |v: &[u128]| {
        let mut s = v.to_vec();
        s.sort_unstable();
        let mean = (s.iter().sum::<u128>() as f64) / (s.len() as f64);
        let median = s[s.len() / 2];
        let p99 = s[(s.len() * 99 / 100).min(s.len() - 1)];
        let min = s[0];
        (mean, median as f64, p99 as f64, min as f64)
    };
    let (cm, cmed, c99, cmin) = stats(&compile_ns);
    let (rm, rmed, r99, rmin) = stats(&render_ns);
    let totals: Vec<u128> = compile_ns
        .iter()
        .zip(&render_ns)
        .map(|(a, b)| a + b)
        .collect();
    let (tm, tmed, t99, tmin) = stats(&totals);

    let audio_seconds = total_ms_sum / 1000.0;
    let total_mean_seconds = tm / 1e9;
    let realtime_factor = audio_seconds / total_mean_seconds;

    println!("input              : {} ({} bytes)", path, bytes);
    println!("iterations         : {}", iters);
    println!(
        "output samples     : {} ({:.2}s of audio at {} Hz)",
        total_samples, audio_seconds, opts.sample_rate
    );
    println!();
    println!("                          mean        median       min          p99");
    println!(
        "compile (us)        : {:>10.1}  {:>10.1}  {:>10.1}  {:>10.1}",
        cm / 1e3,
        cmed / 1e3,
        cmin / 1e3,
        c99 / 1e3
    );
    println!(
        "render  (us)        : {:>10.1}  {:>10.1}  {:>10.1}  {:>10.1}",
        rm / 1e3,
        rmed / 1e3,
        rmin / 1e3,
        r99 / 1e3
    );
    println!(
        "total   (us)        : {:>10.1}  {:>10.1}  {:>10.1}  {:>10.1}",
        tm / 1e3,
        tmed / 1e3,
        tmin / 1e3,
        t99 / 1e3
    );
    println!();
    println!(
        "realtime factor    : {:.1}x  (mean {:.2}ms wall to render {:.2}s of audio)",
        realtime_factor,
        tm / 1e6,
        audio_seconds
    );
}
