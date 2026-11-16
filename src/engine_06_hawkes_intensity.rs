use serde_json::Value;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

// State per asset (BTC=0, ETH=1, SOL=2): (last_timestamp, current_intensity)
static HAWKES_STATE: Mutex<[(f64, f64); 3]> = Mutex::new([(0.0, 1.0); 3]);

// ENGINE 06: Tier-0 Hawkes Process (Self-Exciting Order Arrival Intensity)
pub fn process_tick(parsed: &Value) {
    let msg_type = parsed.get("type").and_then(|t| t.as_str()).unwrap_or("");
    if msg_type != "v2/ticker" && msg_type != "l2_orderbook" {
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

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs_f64();

    if let Ok(mut state) = HAWKES_STATE.lock() {
        let (last_t, prev_lambda) = state[idx];

        let mu = 1.0;    // Baseline arrival rate (शांत मार्केट का बेस)
        let alpha = 1.8; // Self-excitation jump (हर नए टिक का झटका)
        let beta = 2.5;  // Exponential decay rate (ठंडा पड़ने की दर)

        let lambda = if last_t > 0.0 && now >= last_t {
            let dt = now - last_t;
            // Hawkes Stochastic Intensity Formula: λ(t) = μ + (λ_prev - μ) * e^(-β * Δt) + α
            mu + (prev_lambda - mu) * (-beta * dt).exp() + alpha
        } else {
            mu + alpha
        };

        let regime = if lambda > 8.5 {
            "💥 [HAWKES_CASCADE_BURST]"
        } else if lambda > 4.5 {
            "⚡ [EXCITED_ORDER_CLUSTERING]"
        } else {
            "🌊 [POISSON_CALM_FLOW]"
        };

        // टर्मिनल को साफ रखने के लिए सिर्फ v2/ticker आने पर या बर्स्ट होने पर प्रिंट करेंगे
        if msg_type == "v2/ticker" || lambda > 8.5 {
            println!(
                "📈 [E06:{}] HAWKES_λ:{:.2} | EXCITATION:{:.1}x -> {}",
                symbol, lambda, lambda / mu, regime
            );
        }

        state[idx] = (now, lambda);
    }
}
