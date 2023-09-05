// basic config with only logs configuration
// this config can be used by any services in the demo
fn on_config(config) {
    config.broker.uri = "amqps://vsmtp-dev:password@rabbitmq.mydomain.tld:5671/vsmtp-dev"; // Note: all services use the same broker for simplicity
    config.broker.certificate_chain = "/etc/ssl/certs/ca-certificates.crt";

    config.logs.levels = #{
        "receiver": "warn", // FIXME: make a generic way
        "mio": "off",
        "lapin": "off",
        "pinky_swear": "off",
        "hyper": "off",
        "want": "off",
        "polling": "off",
        "async_io": "off",
        "lapin::returned_messages": "off"
    };

    config

}