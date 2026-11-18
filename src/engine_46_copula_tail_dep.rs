use nalgebra::Vector3;
use serde_json::Value;
use std::sync::Mutex;

// State: (last_prices[3], uniform_u[3], kendall_tau[3])
static COPULA_STATE: Mutex<(Vector3<f64>, Vector3<f64>, Vector3<f64>)> = Mutex::new((
    Vector3::new(0.0, 0.0, 0.0),
    Vector3::new(0.5, 0.5, 0.5),
    Vector3::new(0.4, 0.4, 0.4),
));

// ENGINE 46: Tier-0 Clayton & Gumbel Archimedean Copula Asymmetric Tail-Dependence Detector
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

    if let Ok(mut state) = COPULA_STATE.lock() {
        let (mut last_p, mut u_vec, mut tau_vec) = *state;

        if last_p[idx] <= 0.0 {
            last_p[idx] = price;
            *state = (last_p, u_vec, tau_vec);
            return;
        }

        let ret_bps = ((price - last_p[idx]) / last_p[idx]) * 10000.0;
        last_p[idx] = price;

        // Probability Integral Transform (PIT) mapping return to Uniform marginal u in (0.01, 0.99)
        let u_curr = (1.0 / (1.0 + (-1.15 * ret_bps).exp())).clamp(0.01, 0.99);
        let u_btc = u_vec[0];

        // Concordance update for Kendall's Rank Correlation tau between asset and BTC anchor
        let concord = ((u_curr - 0.5) * (u_btc - 0.5) * 4.0).clamp(-0.9, 0.95);
        u_vec[idx] = u_curr;
        tau_vec[idx] = ((tau_vec[idx] * 0.88) + (concord * 0.12)).clamp(0.05, 0.90);

        // Clayton Parameter theta_C = 2*tau / (1 - tau) -> Lower Tail Dependence lambda_L = 2^(-1/theta_C)
        let tau = tau_vec[idx];
        let theta_clayton = ((2.0 * tau) / (1.0 - tau)).max(0.1);
        let lambda_lower = (2.0_f64).powf(-1.0 / theta_clayton);

        // Gumbel Parameter theta_G = 1 / (1 - tau) -> Upper Tail Dependence lambda_U = 2 - 2^(1/theta_G)
        let theta_gumbel = (1.0 / (1.0 - tau)).max(1.02);
        let lambda_upper = (2.0 - (2.0_f64).powf(1.0 / theta_gumbel)).clamp(0.0, 0.99);

        let copula_regime = if u_curr < 0.22 && u_btc < 0.22 && lambda_lower > 0.45 {
            "🩸 [CLAYTON_LOWER_TAIL_CONTAGION_CRASH]"
        } else if u_curr > 0.78 && u_btc > 0.78 && lambda_upper > 0.45 {
            "🚀 [GUMBEL_UPPER_TAIL_EUPHORIA_SQUEEZE]"
        } else {
            "🔗 [COPULA_INDEPENDENT_MARGINALS]"
        };

        println!(
            "🧬 [E46:{}] U_PIT:{:.2} | λ_LOWER(CRASH):{:.2} | λ_UPPER(BOOM):{:.2} -> {}",
            symbol, u_curr, lambda_lower, lambda_upper, copula_regime
        );

        *state = (last_p, u_vec, tau_vec);
    }
}
