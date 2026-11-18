use nalgebra::Vector3;
use serde_json::Value;
use std::sync::Mutex;

// State across [BTC=0, ETH=1, SOL=2]: (last_prices, phase_angles_theta, natural_freq_omega)
static KURAMOTO_STATE: Mutex<(Vector3<f64>, Vector3<f64>, Vector3<f64>)> = Mutex::new((
    Vector3::new(0.0, 0.0, 0.0),
    Vector3::new(0.0, 1.0, 2.0),
    Vector3::new(0.0, 0.0, 0.0),
));

// ENGINE 57: Tier-0 Kuramoto Coupled Oscillator Cross-Asset Phase-Synchronization Detector
pub fn process_tick(parsed: &Value) {
    if parsed.get("type").and_then(|t| t.as_str()) != Some("v2/ticker") {
        return;
    }

    let symbol = match parsed.get("symbol").and_then(|s| s.as_str()) {
        Some(s) => s,
        None => return,
    };

    let idx = if symbol.contains("BTC") {
        0
    } else if symbol.contains("ETH") {
        1
    } else if symbol.contains("SOL") {
        2
    } else {
        return;
    };

    let price = parsed
        .get("mark_price")
        .and_then(|p| p.as_str())
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);

    if price <= 0.0 {
        return;
    }

    if let Ok(mut state) = KURAMOTO_STATE.lock() {
        let (mut last_p, mut theta, mut omega) = *state;

        if last_p[idx] <= 0.0 {
            last_p[idx] = price;
            *state = (last_p, theta, omega);
            return;
        }

        let ret_bps = ((price - last_p[idx]) / last_p[idx]) * 10000.0;
        last_p[idx] = price;

        // Update natural angular frequency omega_i from micro-return velocity
        omega[idx] = (omega[idx] * 0.80) + (ret_bps * 0.20);

        // Kuramoto Coupling Differential Step: d(theta_i)/dt = omega_i + (K/3) * sum_j sin(theta_j - theta_i)
        let k_coupling = 0.45;
        let mut coupling_sum = 0.0;
        for j in 0..3 {
            coupling_sum += (theta[j] - theta[idx]).sin();
        }

        let d_theta = (omega[idx] * 0.15) + ((k_coupling / 3.0) * coupling_sum);
        theta[idx] = (theta[idx] + d_theta) % std::f64::consts::TAU;

        // Global Kuramoto Order Parameter R = sqrt( (mean cos)^2 + (mean sin)^2 ) in [0.0, 1.0]
        let mean_cos = (theta[0].cos() + theta[1].cos() + theta[2].cos()) / 3.0;
        let mean_sin = (theta[0].sin() + theta[1].sin() + theta[2].sin()) / 3.0;
        let sync_r = (mean_cos * mean_cos + mean_sin * mean_sin).sqrt().clamp(0.0, 1.0);
        let collective_vel = (omega[0] + omega[1] + omega[2]) / 3.0;

        let sync_regime = if sync_r > 0.82 && collective_vel > 0.45 {
            "⚡ [KURAMOTO_SUPER_RESONANCE_BULL_LOCK]"
        } else if sync_r > 0.82 && collective_vel < -0.45 {
            "🩸 [KURAMOTO_SYNCHRONIZED_FLASH_CRASH]"
        } else if sync_r < 0.40 {
            "🌀 [KURAMOTO_INCOHERENT_PHASE_DISPERSION]"
        } else {
            "🔗 [KURAMOTO_PARTIAL_COUPLED_DRIFT]"
        };

        println!(
            "🕰️ [E57:{}] SYNC_R:{:.3} | ω_MEAN:{:+.2}bps | θ_PHASE:{:+.2}rad -> {}",
            symbol, sync_r, collective_vel, theta[idx], sync_regime
        );

        *state = (last_p, theta, omega);
    }
}
