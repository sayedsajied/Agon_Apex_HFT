use nalgebra::{Matrix4, Vector4};
use serde_json::Value;
use std::sync::Mutex;

// State per asset (BTC=0, ETH=1, SOL=2): 4-tick rolling price buffer Vector4
static WAVELET_STATE: Mutex<[Vector4<f64>; 3]> =
    Mutex::new([Vector4::new(0.0, 0.0, 0.0, 0.0); 3]);

// ENGINE 23: Tier-0 Discrete Haar Wavelet Transform & Soft-Threshold Tick Denoiser
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

    if let Ok(mut state) = WAVELET_STATE.lock() {
        let prev = state[idx];

        // Initialize 4-tick window on first arrival
        let win = if prev[3] <= 0.0 {
            Vector4::new(price, price, price, price)
        } else {
            Vector4::new(prev[1], prev[2], prev[3], price)
        };
        state[idx] = win;

        // 4-Point Orthogonal Haar Wavelet Transform Matrix (W) via nalgebra
        let s = std::f64::consts::FRAC_1_SQRT_2;
        let w_mat = Matrix4::new(
            0.5,  0.5,  0.5,  0.5, // Coarse Scale Approximation (cA)
            0.5,  0.5, -0.5, -0.5, // Mid-Scale Wavelet Detail (cD2)
              s,   -s,  0.0,  0.0, // Fine-Scale Wavelet Detail (cD1_a)
            0.0,  0.0,    s,   -s, // Fine-Scale Wavelet Detail (cD1_b)
        );

        let mut coeffs = w_mat * win;
        let hf_noise_energy = (coeffs[2].powi(2) + coeffs[3].powi(2)).sqrt();

        // Donoho Universal Soft-Thresholding on Fine-Scale High-Frequency Details
        let thresh = price * 0.00015;
        for i in 2..4 {
            if coeffs[i].abs() <= thresh {
                coeffs[i] = 0.0;
            } else {
                coeffs[i] = coeffs[i].signum() * (coeffs[i].abs() - thresh);
            }
        }

        // Inverse Wavelet Reconstruction: X_denoised = W^T * coeffs
        let denoised_vec = w_mat.transpose() * coeffs;
        let clean_price = denoised_vec[3];
        let edge_bps = ((price - clean_price) / clean_price) * 10000.0;

        let wav_signal = if edge_bps > 1.2 {
            "🟢 [WAVELET_TRUE_BREAKOUT_UP]"
        } else if edge_bps < -1.2 {
            "🔴 [WAVELET_TRUE_BREAKOUT_DOWN]"
        } else {
            "🌊 [WAVELET_NOISE_SCRUBBED]"
        };

        println!(
            "〰️ [E23:{}] RAW:{:.2} | WAV_CLEAN:{:.2} | HF_NOISE:{:.3} -> {}",
            symbol, price, clean_price, hf_noise_energy, wav_signal
        );
    }
}
