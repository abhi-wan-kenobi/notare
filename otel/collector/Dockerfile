FROM otel/opentelemetry-collector-contrib:0.139.0

COPY deploy/otel-collector/config.yaml /etc/otelcol-contrib/config.yaml

CMD ["/otelcol-contrib", "--config", "/etc/otelcol-contrib/config.yaml"]
