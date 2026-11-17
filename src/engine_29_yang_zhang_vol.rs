use nalgebra::Vector3;
use serde_json::Value;
use std::sync::Mutex;

// State per asset (BTC=0, ETH=1, SOL=2): (open_p, high_p, low_p, prev_close, tick_cnt, var_vec[jump, close, rs])
static YZ_STATE: Mutex<[(f64, f64, f64, f64, u32, Vector3<f64>); 3]> = Mutex::new([
    (0.0, 0.0, 0.0, 0.0, 0, Vector3::new(0.2, 0.2, 0.2)),
    (0.0, 0.0, 0.0, 0.0, 0, Vector3::new(0.2, 0.2, 0.2)),
    (0.0, 0.0, 0.0, 0.0, 0, Vector3::new(0.2, 0.2, 0.2)),
]);

// ENGINE 29: Tier-0 Yang-Zhang Minimum-Variance High-Frequency Volatility Estimator
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

    if let Ok(mut state) = YZ_STATE.lock() {
        let (mut open_p, mut high_p, mut low_p, mut prev_c, mut cnt, mut v_vec) = state[idx];

        if prev_c <= 0.0 {
            state[idx] = (price, price, price, price, 1, v_vec);
            return;
        }

        // Update micro-window High/Low excursions over 4-tick micro-bars
        high_p = high_p.max(price);
        low_p = low_p.min(price);
        cnt += 1;

        if cnt >= 4 {
            // Log returns in Basis Points (bps) for Open, High, Low, Close
            let o_jump = ((open_p / prev_c).ln()) * 10000.0;
            let c_drift = ((price / open_p).ln()) * 10000.0;
            let u_high = ((high_p / open_p).ln()) * 10000.0;
            let d_low = ((low_p / open_p).ln()) * 10000.0;

            // Rogers-Satchell (RS) Drift-Independent Variance Component
            let rs_var = (u_high * (u_high - c_drift) + d_low * (d_low - c_drift)).max(0.0);
            let inst_vec = Vector3::new(o_jump * o_jump, c_drift * c_drift, rs_var);
            v_vec = (v_vec * 0.85) + (inst_vec * 0.15);

            // Reset micro-bar window for next 4-tick cycle
            prev_c = price;
            open_p = price;
            high_p = price;
            low_p = price;
            cnt = 0;
        }

        // Yang-Zhang Optimal Weighting Vector: [1.0, k, 1.0 - k] where k = 0.25
        let weights = Vector3::new(1.0, 0.25, 0.75);
        let yz_sigma = v_vec.dot(&weights).max(0.01).sqrt();

        let yz_regime = if yz_sigma > 3.2 {
            "⚡ [YZ_EXTREME_RANGE_EXPANSION]"
        } else if yz_sigma < 0.7 {
            "🔒 [YZ_TIGHT_MICRO_COMPRESSION]"
        } else {
            "⚖️ [YZ_BALANCED_VOLATILITY]"
        };

        println!(
            "📐 [E29:{}] YZ_VOL:{:.2}bps | RS_COMP:{:.2} -> {}",
            symbol, yz_sigma, v_vec[2].sqrt(), yz_regime
        );

        state[idx] = (open_p, high_p, low_p, prev_c, cnt, v_vec);
    }
}
