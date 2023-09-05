fn on_helo(ctx) {
    ctx.run([
        rule "my helo rule" |ctx| {
            hello(); // added by a global module.
            maths::add(2, 2); // added by a static module.

            return status::accept();
        }
    ])
}

fn on_ehlo(ctx) {
    ctx.run([
        rule "my ehlo rule" |ctx| {
            // added by a static module.
            let d = data::new_data();

            d.inc();
            d.inc();
            d.inc();

            print(`data value: ${d.value}`);

            status::next()
        },

        rule "invalid function call" |ctx| {
            xxx();
        }
    ])
}
