package main

import (
	"log"
	"net/http"
	stargate "github.com/syntropika/stargate/packages/go"
)

func main() {
	auth, err := stargate.New(stargate.Config{BaseURL: "https://localhost:3000", Storage: stargate.Turso("./stargate.db"), OIDC: []stargate.OIDC{{Issuer: "https://identity.example.com", ClientID: "your-client-id", ClientSecret: "your-client-secret"}}})
	if err != nil {
		log.Fatal(err)
	}
	defer auth.Close()
	mux := http.NewServeMux()
	mux.Handle("/auth/", auth.Handler())
	mux.Handle("/private", auth.Require(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		_, _ = w.Write([]byte(*stargate.IdentityFromContext(r.Context()).UserID))
	})))
	log.Fatal(http.ListenAndServe("127.0.0.1:3000", auth.Middleware(mux)))
}
