package stargate

import (
	"net/http"
	"net/http/httptest"
	"path/filepath"
	"strings"
	"testing"
)

func TestHTTPAdapter(t *testing.T) {
	auth, err := New(Config{BaseURL: "https://app.example.com", Storage: Turso(filepath.Join(t.TempDir(), "stargate.db"))})
	if err != nil {
		t.Fatal(err)
	}
	defer auth.Close()
	mux := http.NewServeMux()
	mux.Handle("/private", auth.Require(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) { t.Fatal("anonymous request reached private handler") })))
	mux.HandleFunc("/echo", func(w http.ResponseWriter, r *http.Request) {
		var data [9]byte
		_, _ = r.Body.Read(data[:])
		_, _ = w.Write(data[:])
	})
	app := auth.Middleware(mux)
	for _, scenario := range []struct {
		path   string
		status int
	}{{"/auth/", 200}, {"/auth/assets/app.js", 200}, {"/private", 401}, {"/authentication", 404}} {
		t.Run(scenario.path, func(t *testing.T) {
			w := httptest.NewRecorder()
			app.ServeHTTP(w, httptest.NewRequest("GET", scenario.path, nil))
			if w.Code != scenario.status {
				t.Fatalf("unexpected status: %d", w.Code)
			}
		})
	}
	w := httptest.NewRecorder()
	app.ServeHTTP(w, httptest.NewRequest("POST", "/echo", strings.NewReader("host-body")))
	if w.Body.String() != "host-body" {
		t.Fatal("application body was consumed")
	}
	w = httptest.NewRecorder()
	app.ServeHTTP(w, httptest.NewRequest("POST", "/auth/api/keys", strings.NewReader(strings.Repeat("x", 65537))))
	if w.Code != 413 {
		t.Fatal("request limit not enforced")
	}
	if err := auth.Close(); err != nil {
		t.Fatal(err)
	}
	if _, err := auth.Handle(Request{Method: "GET", Path: "/auth/"}); err == nil {
		t.Fatal("closed handle was accepted")
	}
}
