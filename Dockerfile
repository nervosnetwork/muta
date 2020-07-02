FROM mutadev/muta:build-env

LABEL maintainer="yejiayu.fe@gmail.com"

WORKDIR /app

COPY . .
RUN make prod

EXPOSE 1337 8000
CMD ["/app/target/release/examples/muta-chain"]
