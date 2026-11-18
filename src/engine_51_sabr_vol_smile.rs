use nalgebra::Vector3;
use serde_json::Value;
use std::sync::Mutex;

// State per asset (BTC=0, ETH=1, SOL=2): (anchor_f, sabr_vec[alpha_vol, rho_skew, nu_volvol])
static SABR_STATE: Mutex<[(f64, Vector3<f64>); 3]> = Mutex::new([
    (0.0, Vector3::new(1.0, 0.0, 0.5)),
    (0.0, Vector3::new(1.0, 0.0, 0.5)),
    (0.0, Vector3::new(1.0, 0.0, 0.5)),
]);

// ENGINE 51: Tier-0 SABR (Stochastic Alpha-Beta-Rho) Volatility Smile & Skew Curvature Engine
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

    if let Ok(mut state) = SABR_STATE.lock() {
        let (mut anchor_f, prev_sabr) = state[idx];

        if anchor_f <= 0.0 {
            state[idx] = (price, prev_sabr);
            return;
        }

        let ret_bps = ((price - anchor_f) / anchor_f) * 10000.0;
        let abs_ret = ret_bps.abs().max(0.05);
        anchor_f = (anchor_f * 0.90) + (price * 0.10);

        // SABR Parameters: Alpha (base vol), Beta = 0.70 (CEV backbone), Rho (skew), Nu (vol-of-vol smile)
        let alpha = ((prev_sabr[0] * 0.85) + (abs_ret * 0.15)).clamp(0.10, 25.0);
        let d_alpha = abs_ret - prev_sabr[0];
        let corr_inst = (ret_bps.signum() * d_alpha.signum()) * 0.85;
        let rho = ((prev_sabr[1] * 0.88) + (corr_inst * 0.12)).clamp(-0.95, 0.95);
        let nu = ((prev_sabr[2] * 0.85) + ((d_alpha.abs() / alpha) * 0.15)).clamp(0.10, 3.5);

        // Hagan's SABR Skew & Smile Curvature Multiplier z / x(z)
        let log_fk = (price / anchor_f).ln();
        let z = (nu / alpha) * log_fk * 100.0;
        let sabr_skew_shift = (0.5 * rho * nu * z) + ((2.0 - 3.0 * rho * rho) / 24.0) * (nu * nu * z * z);
        let implied_sabr_vol = (alpha * (1.0 + sabr_skew_shift)).max(0.05);

        let next_sabr = Vector3::new(alpha, rho, nu);

        let sabr_regime = if rho > 0.30 && implied_sabr_vol > alpha * 1.05 {
            "📈 [SABR_CALL_SKEW_UPSIDE_RAMP]"
        } else if rho < -0.30 && implied_sabr_vol > alpha * 1.05 {
            "📉 [SABR_PUT_SMIRK_CRASH_HEDGING]"
        } else {
            "⚖️ [SABR_SYMMETRIC_VOL_SMILE]"
        };

        println!(
            "🌈 [E51:{}] α_VOL:{:.2} | ρ_SKEW:{:+.2} | ν_CURV:{:.2} | SABR_σ:{:.2} -> {}",
            symbol, next_sabr[0], next_sabr[1], next_sabr[2], implied_sabr_vol, sabr_regime
        );

        state[idx] = (anchor_f, next_sabr);
    }
}
