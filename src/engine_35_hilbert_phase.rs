use nalgebra::Vector4;
use serde_json::Value;
use std::sync::Mutex;

// State per asset (BTC=0, ETH=1, SOL=2): (last_price, detrended_ret_buffer[4], prev_phase)
static HILBERT_STATE: Mutex<[(f64, Vector4<f64>, f64); 3]> = Mutex::new([
    (0.0, Vector4::new(0.0, 0.0, 0.0, 0.0), 0.0),
    (0.0, Vector4::new(0.0, 0.0, 0.0, 0.0), 0.0),
    (0.0, Vector4::new(0.0, 0.0, 0.0, 0.0), 0.0),
]);

// ENGINE 35: Tier-0 Discrete Hilbert Transform (Instantaneous Phase, Amplitude & Cycle Turn Detector)
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

    if let Ok(mut state) = HILBERT_STATE.lock() {
        let (last_p, prev_buf, prev_phi) = state[idx];

        if last_p <= 0.0 {
            state[idx] = (price, prev_buf, 0.0);
            return;
        }

        let ret_bps = ((price - last_p) / last_p) * 10000.0;
        let buf = Vector4::new(prev_buf[1], prev_buf[2], prev_buf[3], ret_bps);

        // Discrete Hilbert Quadrature FIR Filter Kernel via nalgebra Vector4 dot product
        // In-Phase Component (I) centered at delay tap, Quadrature (Q) 90-degree phase-shifted
        let i_kernel = Vector4::new(0.0, 0.5, 0.5, 0.0);
        let q_kernel = Vector4::new(-0.6366, -0.2122, 0.2122, 0.6366); // 2/pi weighted odd-tap Hilbert kernel

        let in_phase = buf.dot(&i_kernel);
        let quadrature = buf.dot(&q_kernel);

        // Analytic Signal Envelope Amplitude A(t) = sqrt(I^2 + Q^2) & Instantaneous Phase phi(t) = atan2(Q, I)
        let amplitude = (in_phase * in_phase + quadrature * quadrature).sqrt();
        let phase_rad = quadrature.atan2(in_phase);
        let phase_deg = phase_rad.to_degrees();
        let inst_freq = (phase_deg - prev_phi).abs();

        let cycle_regime = if amplitude > 0.8 && phase_deg > -135.0 && phase_deg < -45.0 {
            "🌀 [HILBERT_CYCLE_TROUGH_TURN_UP]"
        } else if amplitude > 0.8 && phase_deg > 45.0 && phase_deg < 135.0 {
            "🌀 [HILBERT_CYCLE_PEAK_TURN_DOWN]"
        } else {
            "〰️ [HILBERT_WAVE_PROPAGATION]"
        };

        println!(
            "📡 [E35:{}] AMP:{:.2}bps | PHASE:{:+.1}° | ω_FREQ:{:.1}°/t -> {}",
            symbol, amplitude, phase_deg, inst_freq, cycle_regime
        );

        state[idx] = (price, buf, phase_deg);
    }
}
