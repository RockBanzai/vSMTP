fn on_mutate_state(ctx) {
    ctx.run([
        action "mutate state 1" |ctx| ctx.inc(),
        action "mutate state 2" |ctx| ctx.inc(),
        action "mutate state 3" |ctx| ctx.inc(),
        action "mutate state 4" |ctx| ctx.inc(),
        action "mutate state 5" |ctx| ctx.inc(),
        rule   "trailing" |ctx| status::next("state mutated"),
    ])
}

fn on_fetch_state(ctx) {
    ctx.run([
        rule "check state value" |ctx| {
            if ctx.value == 5 {
                status::next("state fetched")
            } else {
                status::stop()
            }
        },
    ])
}