use nalgebra::Vector4;
use serde_json::Value;
use std::sync::Mutex;

// State per asset (BTC=0, ETH=1, SOL=2): (last_price, bin_counts[4])
static ENTROPY_STATE: Mutex<[(f64, Vector4<f64>); 3]> =
    Mutex::new([(0.0, Vector4::new(1.0, 1.0, 1.0, 1.0)); 3]);

// ENGINE 22: Tier-0 Shannon Entropy Market Noise & Information Filter
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

    if let Ok(mut state) = ENTROPY_STATE.lock() {
        let (last_p, mut bins) = state[idx];

        if last_p <= 0.0 {
            state[idx] = (price, Vector4::new(1.0, 1.0, 1.0, 1.0));
            return;
        }

        let ret_bps = ((price - last_p) / last_p) * 10000.0;

        // Categorize return into 4 discrete probability bins via nalgebra indexing
        bins *= 0.90; // Exponential decay on older observations
        if ret_bps < -1.5 {
            bins[0] += 1.0;
        } else if ret_bps < 0.0 {
            bins[1] += 1.0;
        } else if ret_bps <= 1.5 {
            bins[2] += 1.0;
        } else {
            bins[3] += 1.0;
        }

        let total_counts = bins.sum().max(1.0);
        let p = bins / total_counts;

        // Shannon Entropy Formula: H = - sum(p_i * log2(p_i))
        let mut entropy = 0.0;
        for i in 0..4 {
            if p[i] > 0.001 {
                entropy -= p[i] * p[i].log2();
            }
        }
        // Normalized entropy between 0.0 (Pure order) and 1.0 (Maximum random noise)
        let norm_entropy = (entropy / 2.0).clamp(0.0, 1.0);

        let noise_signal = if norm_entropy > 0.85 {
            "🔕 [HIGH_NOISE_RANDOM_WALK_BLOCK]"
        } else if norm_entropy < 0.50 {
            "🎯 [LOW_NOISE_DETERMINISTIC_FLOW]"
        } else {
            "⚖️ [MODERATE_ENTROPY_ZONE]"
        };

        println!(
            "📊 [E22:{}] RET:{:+.2}bps | ENTROPY_H:{:.3} -> {}",
            symbol, ret_bps, norm_entropy, noise_signal
        );

        state[idx] = (price, bins);
    }
}
