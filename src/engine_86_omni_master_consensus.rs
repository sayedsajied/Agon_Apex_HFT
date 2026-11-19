use nalgebra::Vector4;
use serde_json::Value;
use std::sync::Mutex;

// State per asset (BTC=0, ETH=1, SOL=2): (last_price, omni_vec4[micro, thermo, quantum, gravity], var_ema)
static OMNI_MASTER_STATE: Mutex<[(f64, Vector4<f64>, f64); 3]> = Mutex::new([
    (0.0, Vector4::new(0.0, 0.0, 0.0, 0.0), 1.0),
    (0.0, Vector4::new(0.0, 0.0, 0.0, 0.0), 1.0),
    (0.0, Vector4::new(0.0, 0.0, 0.0, 0.0), 1.0),
]);

// ENGINE 86 (GRAND FINALE): Tier-0 Omni-Physics Master Consensus, Coherence & Kelly Execution Gate
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

    let quotes = parsed.get("quotes");
    let best_bid = quotes
        .and_then(|q| q.get("best_bid"))
        .and_then(|b| b.as_str())
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(price);
    let best_ask = quotes
        .and_then(|q| q.get("best_ask"))
        .and_then(|a| a.as_str())
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(price);
    let bid_sz = quotes
        .and_then(|q| q.get("bid_size"))
        .and_then(|b| b.as_str())
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(1.0);
    let ask_sz = quotes
        .and_then(|q| q.get("ask_size"))
        .and_then(|a| a.as_str())
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(1.0);

    if price <= 0.0 {
        return;
    }

    if let Ok(mut state) = OMNI_MASTER_STATE.lock() {
        let (last_p, prev_psi, prev_var) = state[idx];

        if last_p <= 0.0 {
            state[idx] = (price, prev_psi, prev_var);
            return;
        }

        let ret_bps = ((price - last_p) / last_p) * 10000.0;
        let spread_bps = (((best_ask - best_bid).max(0.0) / price) * 10000.0).clamp(0.05, 25.0);
        let book_imb = (bid_sz - ask_sz) / (bid_sz + ask_sz).max(0.1);
        let depth_log = (bid_sz + ask_sz).max(1.0).ln().clamp(0.4, 8.0);

        // 4-Domain Omni-Physics Consensus Tensor Psi = [Microstructure, Thermo/Entropy, Quantum, Gravity]
        let s_micro = (book_imb * 1.45).tanh();
        let s_thermo = (ret_bps / (prev_var.sqrt().max(0.35) * 1.5)).tanh();
        let s_quantum = ((ret_bps * 0.60 + book_imb * 0.90) / spread_bps.sqrt()).tanh();
        let s_gravity = ((ret_bps * depth_log * 0.20) + (book_imb * 0.85)).tanh();

        let inst_psi = Vector4::new(s_micro, s_thermo, s_quantum, s_gravity);
        let psi = (prev_psi * 0.68) + (inst_psi * 0.32);

        // Weighted Master Conviction & Cross-Domain Phase Coherence C in [0, 1]
        let weights = Vector4::new(0.28, 0.24, 0.26, 0.22);
        let master_score = psi.dot(&weights).clamp(-1.0, 1.0);
        let sum_abs = psi.iter().map(|v| v.abs()).sum::<f64>().max(1e-6);
        let coherence = (psi.sum().abs() / sum_abs).clamp(0.0, 1.0);

        // Variance update & Fractional Quarter-Kelly Position Sizing Fraction
        let var_next = ((prev_var * 0.85) + (ret_bps * ret_bps * 0.15)).clamp(0.10, 40.0);
        let kelly_size = ((master_score.abs() * coherence) / var_next.sqrt())
            .clamp(0.0, 1.0)
            * 0.25;

        let exec_signal = if spread_bps > 4.5 {
            "🛡️ [OMNI_SPREAD_VETO_BLOCKED]"
        } else if master_score > 0.52 && coherence > 0.78 {
            "🟢👑 [OMNI_MASTER_EXECUTE_LONG]"
        } else if master_score < -0.52 && coherence > 0.78 {
            "🔴👑 [OMNI_MASTER_EXECUTE_SHORT]"
        } else if coherence < 0.45 {
            "⚠️ [OMNI_DOMAIN_CONFLICT_NO_TRADE]"
        } else {
            "⚖️ [OMNI_CONSENSUS_SCANNING]"
        };

        println!(
            "👑 [E86:{}] SCORE:{:+.3} | COH:{:.1}% | KELLY:{:.1}% | SPRD:{:.2}bps -> {}",
            symbol,
            master_score,
            coherence * 100.0,
            kelly_size * 100.0,
            spread_bps,
            exec_signal
        );

        state[idx] = (price, psi, var_next);
    }
}
