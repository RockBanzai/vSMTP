fn on_config_check(ctx) {
    ctx.run([
        rule "configure delivery" |ctx| {
            if config.bar.x is 15 && config.bar.y is 50 {
                status::next("configuration successful")
            } else {
                status::stop()
            }
        }
    ])
}
