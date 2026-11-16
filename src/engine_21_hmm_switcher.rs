use nalgebra::{Matrix2, Vector2};
use serde_json::Value;
use std::sync::Mutex;

// State per asset (BTC=0, ETH=1, SOL=2): (last_price, prob_calm, prob_burst)
static HMM_STATE: Mutex<[(f64, f64, f64); 3]> = Mutex::new([(0.0, 0.85, 0.15); 3]);

// ENGINE 21: Tier-0 Hidden Markov Model (HMM) Bayesian Regime Switcher
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

    if let Ok(mut state) = HMM_STATE.lock() {
        let (last_p, p_calm, p_burst) = state[idx];

        if last_p <= 0.0 {
            state[idx] = (price, 0.85, 0.15);
            return;
        }

        let ret_bps = ((price - last_p) / last_p) * 10000.0;
        let r_sq = ret_bps * ret_bps;

        // Markov Transition Matrix T^T via nalgebra: [P(0->0)=0.92, P(1->0)=0.20; P(0->1)=0.08, P(1->1)=0.80]
        let trans_t = Matrix2::new(0.92, 0.20, 0.08, 0.80);
        let prev_prob = Vector2::new(p_calm, p_burst);
        let prior = trans_t * prev_prob;

        // Gaussian Emission Likelihoods for Calm (sigma=0.6 bps) vs Burst (sigma=3.5 bps)
        let l_calm = (-r_sq / (2.0 * 0.36)).exp() / 0.6;
        let l_burst = (-r_sq / (2.0 * 12.25)).exp() / 3.5;

        let unnorm = Vector2::new(prior[0] * l_calm, prior[1] * l_burst);
        let norm_sum = (unnorm[0] + unnorm[1]).max(1e-12);
        let post = unnorm / norm_sum;

        let hmm_regime = if post[1] > 0.65 {
            "🔥 [HMM_STATE_1: BURST_REGIME_LOCKED]"
        } else if post[1] > 0.35 {
            "⚡ [HMM_TRANSITION_PHASE]"
        } else {
            "🧊 [HMM_STATE_0: CALM_SCALP_REGIME]"
        };

        println!(
            "🔮 [E21:{}] P(CALM):{:.1}% | P(BURST):{:.1}% -> {}",
            symbol, post[0] * 100.0, post[1] * 100.0, hmm_regime
        );

        state[idx] = (price, post[0], post[1]);
    }
}
