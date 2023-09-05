// Syntax using the email "flow".
export const rules = #{
    inbound: [
        rule "inbound analysis" |ctx| {
            // anti-virus, anti-span etc.
            status::ok("250 your email has been analyzed")
        },
    ],

    outbound: [
        rule "authentication" |ctx| {
            // auth ...
            prel::ok_outbound()
        },
    ],

    local: [
        rule "local" |ctx| {
            status::ok("250 All locals are accepted")
        },
    ],
};
