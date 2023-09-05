export const rules = #{

    mail_from: [
        rule "do stuff" |ctx| status::ok(),        
    ],

    rcpt_to: [
        rule "setup basic delivery ..." |ctx| status::ok(),
    ],

};