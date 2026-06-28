use anyhow::{Result, anyhow};
use chrono::{Datelike, Duration, Local, NaiveDate};
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub struct Quote {
    pub date: NaiveDate,
    pub usd_brl: f64,
}

#[derive(Debug, Deserialize)]
struct BcbResponse {
    value: Vec<BcbQuote>,
}

#[derive(Debug, Deserialize)]
struct BcbQuote {
    #[serde(rename = "cotacaoVenda")]
    cotacao_venda: f64,
    #[serde(rename = "dataHoraCotacao")]
    data_hora_cotacao: String,
}

pub fn fetch_bcb_usd_brl_quotes(last_n: usize) -> Result<Vec<Quote>> {
    let end = Local::now().date_naive();
    let start = end - Duration::days(75);

    let url = format!(
        "https://olinda.bcb.gov.br/olinda/servico/PTAX/versao/v1/odata/\
         CotacaoDolarPeriodo(dataInicial='{}',dataFinalCotacao='{}')?$top=180&$format=json",
        bcb_date(start),
        bcb_date(end)
    );

    let resp: BcbResponse = reqwest::blocking::get(url)?
        .error_for_status()?
        .json()?;

    let mut by_day = BTreeMap::<NaiveDate, f64>::new();
    for q in resp.value {
        let date = NaiveDate::parse_from_str(&q.data_hora_cotacao[..10], "%Y-%m-%d")?;
        by_day.insert(date, q.cotacao_venda);
    }

    let mut quotes: Vec<Quote> = by_day
        .into_iter()
        .map(|(date, usd_brl)| Quote { date, usd_brl })
        .collect();

    if quotes.len() > last_n {
        quotes = quotes.split_off(quotes.len() - last_n);
    }

    if quotes.len() < 8 {
        return Err(anyhow!("BCB returned too few quotes"));
    }

    Ok(quotes)
}

fn bcb_date(d: NaiveDate) -> String {
    format!("{:02}-{:02}-{:04}", d.month(), d.day(), d.year())
}

pub fn synthetic_quotes() -> Vec<Quote> {
    let start = NaiveDate::from_ymd_opt(2026, 5, 4).unwrap_or(NaiveDate::MIN);
    let values = [
        5.64, 5.66, 5.61, 5.73, 5.76, 5.74, 5.62, 5.60, 5.67, 5.70, 5.58, 5.56, 5.59, 5.71, 5.69, 5.65, 5.77, 5.80, 5.68, 5.66, 5.72, 5.84, 5.82, 5.79, 5.67, 5.69, 5.74, 5.63, 5.65, 5.78,
    ];

    values
        .iter()
        .enumerate()
        .map(|(i, usd_brl)| Quote { date: start + Duration::days(i as i64), usd_brl: *usd_brl })
        .collect()
}
