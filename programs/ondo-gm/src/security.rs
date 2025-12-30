use solana_security_txt::security_txt;

#[cfg(not(feature = "no-entrypoint"))]
security_txt! {
    name: "Ondo Global Markets Program",
    project_url: "https://ondo.finance",
    contacts: "email:security@ondo.finance",
    policy: "https://immunefi.com/bug-bounty/ondofinance/information/",
    source_code: "https://github.com/ondoprotocol/global-markets-solana",
    auditors: "https://docs.ondo.finance/audits"
}
