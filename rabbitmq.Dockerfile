FROM rabbitmq:3.12-management

ARG DELAYED=rabbitmq_delayed_message_exchange-3.12.0.ez
ARG PLUGIN_URL=https://github.com/rabbitmq/rabbitmq-delayed-message-exchange/releases/download/v3.12.0

RUN apt update && apt install -y curl
RUN curl -L $PLUGIN_URL/$DELAYED --output /opt/rabbitmq/plugins/$DELAYED
RUN chown rabbitmq:rabbitmq /opt/rabbitmq/plugins/$DELAYED

RUN rabbitmq-plugins enable rabbitmq_delayed_message_exchange
