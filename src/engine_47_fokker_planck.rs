use nalgebra::Vector4;
use serde_json::Value;
use std::sync::Mutex;

// State per asset (BTC=0, ETH=1, SOL=2): (last_price, rolling_mu, rolling_var, prob_density_vec4)
static FP_STATE: Mutex<[(f64, f64, f64, Vector4<f64>); 3]> = Mutex::new([
    (0.0, 0.0, 1.0, Vector4::new(0.25, 0.25, 0.25, 0.25)),
    (0.0, 0.0, 1.0, Vector4::new(0.25, 0.25, 0.25, 0.25)),
    (0.0, 0.0, 1.0, Vector4::new(0.25, 0.25, 0.25, 0.25)),
]);

// ENGINE 47: Tier-0 Fokker-Planck Probability Density Evolution & Kolmogorov Forward PDE Solver
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

    if let Ok(mut state) = FP_STATE.lock() {
        let (last_p, prev_mu, prev_var, mut p_grid) = state[idx];

        if last_p <= 0.0 {
            state[idx] = (price, 0.0, 1.0, p_grid);
            return;
        }

        let ret_bps = ((price - last_p) / last_p) * 10000.0;
        let mu = (prev_mu * 0.85) + (ret_bps * 0.15);
        let diff = ret_bps - mu;
        let d_coeff = ((prev_var * 0.85) + (diff * diff * 0.15)).clamp(0.05, 10.0);

        // Discretized Fokker-Planck PDE Operator across 4 state bins: [-2, -1, +1, +2]
        // dP/dt = -d/dx(mu * P) + 0.5 * d^2/dx^2(D * P)
        let adv = (mu * 0.08).clamp(-0.20, 0.20);
        let diff_rate = (d_coeff * 0.02).clamp(0.01, 0.15);

        let dp0 = (-adv * p_grid[0]) + diff_rate * (p_grid[1] - p_grid[0]);
        let dp1 = (adv * (p_grid[0] - p_grid[1])) + diff_rate * (p_grid[0] - 2.0 * p_grid[1] + p_grid[2]);
        let dp2 = (adv * (p_grid[1] - p_grid[2])) + diff_rate * (p_grid[1] - 2.0 * p_grid[2] + p_grid[3]);
        let dp3 = (adv * p_grid[2]) + diff_rate * (p_grid[2] - p_grid[3]);

        p_grid += Vector4::new(dp0, dp1, dp2, dp3);
        p_grid = p_grid.map(|v| v.max(0.01));
        p_grid /= p_grid.sum();

        // Probability Mass Skew between Upper Bull States (bins 2,3) and Lower Bear States (bins 0,1)
        let bull_mass = p_grid[2] + p_grid[3];
        let bear_mass = p_grid[0] + p_grid[1];
        let fp_flux = bull_mass - bear_mass;

        let fp_regime = if fp_flux > 0.24 {
            "🌊 [FP_BULLISH_DENSITY_WAVE_ADVECTION]"
        } else if fp_flux < -0.24 {
            "🌊 [FP_BEARISH_DENSITY_WAVE_ADVECTION]"
        } else {
            "⚖️ [FP_STATIONARY_EQUILIBRIUM_PDF]"
        };

        println!(
            "🌌 [E47:{}] P_BULL:{:.1}% | P_BEAR:{:.1}% | FP_FLUX:{:+.3} -> {}",
            symbol,
            bull_mass * 100.0,
            bear_mass * 100.0,
            fp_flux,
            fp_regime
        );

        state[idx] = (price, mu, d_coeff, p_grid);
    }
}
