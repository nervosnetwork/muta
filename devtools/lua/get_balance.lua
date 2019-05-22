wrk.method = "POST"
wrk.headers["Content-Type"] = "application/json"
wrk.body = '{"jsonrpc":"2.0","method":"getBalance","params":["0x19e49d3efd4e81dc82943ad9791c1916e2229138", "latest"],"id":1}'