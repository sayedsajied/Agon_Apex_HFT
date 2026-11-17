use nalgebra::{Matrix2, Vector2};
use serde_json::Value;
use std::sync::Mutex;

// State per asset (BTC=0, ETH=1, SOL=2): (last_price, nash_vec[taker_agg_prob, mm_def_prob])
static NASH_STATE: Mutex<[(f64, Vector2<f64>); 3]> = Mutex::new([
    (0.0, Vector2::new(0.5, 0.5)),
    (0.0, Vector2::new(0.5, 0.5)),
    (0.0, Vector2::new(0.5, 0.5)),
]);

// ENGINE 42: Tier-0 Nash Equilibrium Bimatrix Game Theory (Predator Taker vs Prey Maker)
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

    if let Ok(mut state) = NASH_STATE.lock() {
        let (last_p, prev_nash) = state[idx];

        if last_p <= 0.0 {
            state[idx] = (price, prev_nash);
            return;
        }

        let ret_bps = ((price - last_p) / last_p).abs() * 10000.0;
        let spread_bps = (((best_ask - best_bid).max(0.0) / price) * 10000.0).max(0.2);
        let depth_imb = ((bid_sz - ask_sz) / (bid_sz + ask_sz).max(0.1)).abs();

        // 2x2 Payoff Matrix M for Taker Strategies [Sweep, Wait] vs Maker Strategies [Hold, Retreat]
        let m11 = ret_bps - (0.5 * spread_bps); // Sweep into Hold (Predator captures informed edge)
        let m12 = -spread_bps;                  // Sweep into Retreat (Predator pays wide spread cost)
        let m21 = -0.3 * ret_bps;               // Wait while Hold (Missed alpha decay)
        let m22 = depth_imb * 1.5;              // Wait while Retreat (Tactical book reset)

        let payoff_mat = Matrix2::new(m11, m12, m21, m22);
        let denom = (m11 - m12 - m21 + m22).abs().max(0.1);

        // Mixed-Strategy Nash Equilibrium Indifference Probabilities via nalgebra
        let p_sweep = ((m22 - m21).abs() / denom).clamp(0.05, 0.95);
        let q_retreat = ((m22 - m12).abs() / denom).clamp(0.05, 0.95);

        let inst_nash = Vector2::new(p_sweep, q_retreat);
        let smooth_nash = (prev_nash * 0.75) + (inst_nash * 0.25);

        // Expected Game Value V = p^T * M * q
        let p_vec = Vector2::new(smooth_nash[0], 1.0 - smooth_nash[0]);
        let q_vec = Vector2::new(1.0 - smooth_nash[1], smooth_nash[1]);
        let game_val = p_vec.dot(&(payoff_mat * q_vec));

        let game_regime = if smooth_nash[0] > 0.65 && smooth_nash[1] > 0.60 {
            "🦈 [NASH_PREDATOR_DOMINANCE_SWEEP]"
        } else if smooth_nash[0] < 0.38 && smooth_nash[1] < 0.40 {
            "🛡️ [NASH_MAKER_FORTIFIED_EQUILIBRIUM]"
        } else {
            "♟️ [NASH_MIXED_STRATEGY_STANDOFF]"
        };

        println!(
            "♟️ [E42:{}] P(SWEEP):{:.1}% | Q(RETREAT):{:.1}% | GAME_V:{:+.2} -> {}",
            symbol,
            smooth_nash[0] * 100.0,
            smooth_nash[1] * 100.0,
            game_val,
            game_regime
        );

        state[idx] = (price, smooth_nash);
    }
}
