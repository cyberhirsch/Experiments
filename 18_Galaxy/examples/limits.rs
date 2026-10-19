//! Headless probe of what this machine's GPU will actually allocate, and what
//! that means for the PRD's 1e9-system target. No window, no surface - just
//! the adapter. Run with `cargo run --example limits`.

use galaxy_colonization::galaxy::{Budget, BYTES_EVENT, BYTES_POS, BYTES_STATE};

fn gib(v: u64) -> f64 {
    v as f64 / (1024.0 * 1024.0 * 1024.0)
}

fn main() {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: None,
        force_fallback_adapter: false,
        apply_limit_buckets: false,
    }))
    .expect("no adapter");

    let info = adapter.get_info();
    let l = adapter.limits();
    println!("adapter                    {} ({:?})", info.name, info.backend);
    println!("driver                     {} {}", info.driver, info.driver_info);
    println!();
    println!("max_buffer_size            {:>10.2} GiB", gib(l.max_buffer_size));
    println!(
        "max_storage_binding        {:>10.2} GiB",
        gib(l.max_storage_buffer_binding_size as u64)
    );
    println!(
        "max_compute_workgroups/dim {:>10}",
        l.max_compute_workgroups_per_dimension
    );
    println!();

    let bind = (l.max_storage_buffer_binding_size as u64).min(l.max_buffer_size);
    let per_buf = bind / BYTES_POS;
    println!("position buffer holds      {:>10.1} M systems", per_buf as f64 / 1e6);
    println!(
        "1e9 systems needs          {:>10} position buffer(s)",
        1_000_000_000u64.div_ceil(per_buf.max(1))
    );
    // One dispatch covers workgroups * 256 invocations.
    let per_dispatch = l.max_compute_workgroups_per_dimension as u64 * 256;
    println!(
        "one dispatch covers        {:>10.1} M systems",
        per_dispatch as f64 / 1e6
    );
    println!();

    println!("--- memory, by system count ---");
    println!("           total     substrate  event clock  spatial hash");
    for n in [100_000_000u64, 250_000_000, 500_000_000, 1_000_000_000] {
        let b = Budget::for_count(n);
        println!(
            "{:>5} M  {:>7.2} GiB  {:>7.2} GiB  {:>7.2} GiB  {:>7.2} GiB",
            n / 1_000_000,
            gib(b.total),
            gib(b.substrate),
            gib(b.events),
            gib(b.hash),
        );
    }
    println!();

    // What the PRD's own stated choices would cost: f64 years for time
    // (Section 8 offers "f64 or integer years") and a conventional spatial
    // hash carrying its own u32 index array alongside the points.
    let n = 1_000_000_000u64;
    let prd_sub = n * (BYTES_POS + BYTES_STATE);
    let prd_evt = n * 8; // f64 years
    let prd_idx = n * 4; // separate sort index
    let prd_cells = (n / 32).max(1) * 2 * 8;
    let prd_total = prd_sub + prd_evt + prd_idx + prd_cells;
    println!("--- 1e9 systems: two design choices decide whether it fits ---");
    println!(
        "f64 years + indexed hash   {:>7.2} GiB   {}",
        gib(prd_total),
        if prd_total > 24 * 1024 * 1024 * 1024 { "OVER a 24 GiB card" } else { "fits" }
    );
    let ours = Budget::for_count(n).total;
    println!(
        "u32 decades + sorted hash  {:>7.2} GiB   {}",
        gib(ours),
        if ours > 24 * 1024 * 1024 * 1024 { "OVER a 24 GiB card" } else { "fits" }
    );
    println!(
        "                           {:>7.2} GiB saved by those two choices alone",
        gib(prd_total - ours)
    );
    let _ = BYTES_STATE;
    let _ = BYTES_EVENT;
}
