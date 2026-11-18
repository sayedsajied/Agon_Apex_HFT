use nalgebra::Vector4;
use serde_json::Value;
use std::sync::Mutex;

// State per asset (BTC=0, ETH=1, SOL=2): (last_price, rolling_mu, rolling_sigma, path_weights_vec4)
static FK_STATE: Mutex<[(f64, f64, f64, Vector4<f64>); 3]> = Mutex::new([
    (0.0, 0.0, 1.0, Vector4::new(0.25, 0.25, 0.25, 0.25)),
    (0.0, 0.0, 1.0, Vector4::new(0.25, 0.25, 0.25, 0.25)),
    (0.0, 0.0, 1.0, Vector4::new(0.25, 0.25, 0.25, 0.25)),
]);

// ENGINE 48: Tier-0 Feynman-Kac Stochastic PDE Path-Integral Expectancy Pricer
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

    if let Ok(mut state) = FK_STATE.lock() {
        let (last_p, prev_mu, prev_sig, prev_w) = state[idx];

        if last_p <= 0.0 {
            state[idx] = (price, 0.0, 1.0, prev_w);
            return;
        }

        let ret_bps = ((price - last_p) / last_p) * 10000.0;
        let mu = (prev_mu * 0.85) + (ret_bps * 0.15);
        let sigma = ((prev_sig * 0.85) + ((ret_bps - mu).abs() * 0.15)).max(0.20);

        // 4 Canonical Micro-Horizon Trajectory shocks in bps: [-2*sigma, -0.5*sigma, +0.5*sigma, +2*sigma]
        let paths = Vector4::new(-2.0 * sigma, -0.5 * sigma, 0.5 * sigma, 2.0 * sigma);

        // Feynman-Kac Radon-Nikodym & Onsager-Machlup Path Action Weight: exp( -(x - mu)^2 / (2*sigma^2) - V(x) )
        let two_var = 2.0 * sigma * sigma;
        let potential_v = (ret_bps.abs() * 0.02).clamp(0.0, 0.5); // Stochastic killing/discount rate r(X_s)

        let raw_weights = paths.map(|x| {
            let action = ((x - mu) * (x - mu)) / two_var;
            (-action - potential_v).exp()
        });

        let norm_w = raw_weights / raw_weights.sum().max(1e-6);
        let smooth_w = (prev_w * 0.70) + (norm_w * 0.30);

        // Expected Feynman-Kac Path-Integral Terminal Payoff E^Q[ e^{-int r ds} * Phi(X_T) ] in bps
        let fk_expectancy_bps = smooth_w.dot(&paths);

        let fk_regime = if fk_expectancy_bps > 0.45 {
            "⚛️ [FK_PATH_INTEGRAL_BULL_CONVERGENCE]"
        } else if fk_expectancy_bps < -0.45 {
            "⚛️ [FK_PATH_INTEGRAL_BEAR_CONVERGENCE]"
        } else {
            "⚖️ [FK_QUANTUM_PATH_EQUILIBRIUM]"
        };

        println!(
            "🔮 [E48:{}] FK_E[ΔP]:{:+.2}bps | P(TAIL_UP):{:.1}% | P(TAIL_DN):{:.1}% -> {}",
            symbol,
            fk_expectancy_bps,
            smooth_w[3] * 100.0,
            smooth_w[0] * 100.0,
            fk_regime
        );

        state[idx] = (price, mu, sigma, smooth_w);
    }
}
