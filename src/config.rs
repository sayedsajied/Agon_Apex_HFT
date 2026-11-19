#![allow(dead_code)]

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExchangeId {
    DeltaExchange,
    Bybit,
    Binance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TradingMode {
    Paper,
    Live,
}

#[derive(Debug, Clone)]
pub struct FeeConfig {
    pub maker_fee_bps: f64,
    pub taker_fee_bps: f64,
    pub slippage_bps: f64,
}

impl FeeConfig {
    /// Total round-trip taker cost in basis points (entry + exit + slippage)
    pub fn round_trip_taker_bps(&self) -> f64 {
        (self.taker_fee_bps * 2.0) + (self.slippage_bps * 2.0)
    }

    /// Total round-trip maker cost in basis points
    pub fn round_trip_maker_bps(&self) -> f64 {
        (self.maker_fee_bps * 2.0) + self.slippage_bps
    }
}

#[derive(Debug, Clone)]
pub struct RiskConfig {
    pub max_daily_loss_pct: f64,
    pub max_position_usd: f64,
    pub max_leverage: f64,
    pub max_spread_bps: f64,
    pub stale_feed_ms: u64,
    pub trade_cooldown_ms: u64,
    pub min_edge_to_fee_ratio: f64,
}

#[derive(Debug, Clone)]
pub struct BotConfig {
    pub exchange: ExchangeId,
    pub mode: TradingMode,
    pub symbols: Vec<String>,
    pub initial_capital_usd: f64,
    pub fees: FeeConfig,
    pub risk: RiskConfig,
    pub api_key: String,
    pub api_secret: String,
    pub telegram_bot_token: String,
    pub telegram_chat_id: String,
    pub health_port: u16,
}

impl Default for BotConfig {
    fn default() -> Self {
        Self {
            exchange: ExchangeId::DeltaExchange,
            mode: TradingMode::Paper,
            symbols: vec!["BTCUSD".to_string(), "PAXGUSD".to_string()],
            initial_capital_usd: 100.0,
            fees: FeeConfig {
                maker_fee_bps: 2.0, // 0.02%
                taker_fee_bps: 5.0, // 0.05%
                slippage_bps: 1.5,  // 0.015% realistic live buffer
            },
            risk: RiskConfig {
                max_daily_loss_pct: 3.0,
                max_position_usd: 25.0,
                max_leverage: 3.0,
                max_spread_bps: 4.5,
                stale_feed_ms: 3000,
                trade_cooldown_ms: 15_000,
                min_edge_to_fee_ratio: 3.0,
            },
            api_key: String::new(),
            api_secret: String::new(),
            telegram_bot_token: String::new(),
            telegram_chat_id: String::new(),
            health_port: 10000,
        }
    }
}
