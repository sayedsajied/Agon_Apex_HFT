use nalgebra::Vector3;
use serde_json::Value;
use std::sync::Mutex;

// State per asset (BTC=0, ETH=1, SOL=2): (anchor_mu, prev_spread_x, ou_vec[theta_speed, sigma_eq, ou_zscore])
static OU_STATE: Mutex<[(f64, f64, Vector3<f64>); 3]> = Mutex::new([
    (0.0, 0.0, Vector3::new(0.25, 1.0, 0.0)),
    (0.0, 0.0, Vector3::new(0.25, 1.0, 0.0)),
    (0.0, 0.0, Vector3::new(0.25, 1.0, 0.0)),
]);

// ENGINE 68: Tier-0 Ornstein-Uhlenbeck (OU) Mean-Reverting Velocity & Half-Life Decay Engine
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

    if let Ok(mut state) = OU_STATE.lock() {
        let (mut mu_anchor, prev_x, prev_ou) = state[idx];

        if mu_anchor <= 0.0 {
            state[idx] = (price, 0.0, prev_ou);
            return;
        }

        // Deviation from dynamic equilibrium anchor in basis points: X_t = ((P_t - mu) / mu) * 10000
        let x_curr = ((price - mu_anchor) / mu_anchor) * 10000.0;
        let dx = x_curr - prev_x;
        mu_anchor = (mu_anchor * 0.94) + (price * 0.06);

        // Estimate OU Mean-Reversion Stiffness theta from restoring pull (-dx / prev_x)
        let inst_pull = if prev_x.abs() > 0.15 {
            (-dx / prev_x).clamp(0.02, 1.50)
        } else {
            prev_ou[0]
        };
        let theta = ((prev_ou[0] * 0.86) + (inst_pull * 0.14)).clamp(0.05, 1.20);

        // OU Stationary Equilibrium Volatility: sigma_eq = sigma_diff / sqrt(2 * theta)
        let resid_shock = (dx + (theta * prev_x)).abs();
        let sigma_diff = ((prev_ou[1] * 0.85) + (resid_shock * 0.15)).clamp(0.10, 20.0);
        let sigma_eq = (sigma_diff / (2.0 * theta).sqrt()).max(0.10);

        // Exact Ornstein-Uhlenbeck Stationary Z-Score & Half-Life tau_1/2 = ln(2) / theta
        let ou_z = (x_curr / sigma_eq).clamp(-5.0, 5.0);
        let half_life = std::f64::consts::LN_2 / theta;
        let next_ou = Vector3::new(theta, sigma_diff, ou_z);

        let ou_regime = if ou_z < -1.85 && theta > 0.18 {
            "🟢 [OU_OVERSOLD_ELASTIC_SNAPBACK_LONG]"
        } else if ou_z > 1.85 && theta > 0.18 {
            "🔴 [OU_OVERBOUGHT_ELASTIC_SNAPBACK_SHORT]"
        } else if theta < 0.09 && ou_z.abs() > 2.2 {
            "⚡ [OU_HALFLIFE_BLOWOUT_TREND_ESCAPE]"
        } else {
            "⚖️ [OU_STATIONARY_EQUILIBRIUM_ORBIT]"
        };

        println!(
            "🧲 [E68:{}] OU_Z:{:+.2} | θ_PULL:{:.3} | τ_HALF:{:.1}tks -> {}",
            symbol, next_ou[2], next_ou[0], half_life, ou_regime
        );

        state[idx] = (mu_anchor, x_curr, next_ou);
    }
}
