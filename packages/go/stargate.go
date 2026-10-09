// Package stargate adapts net/http to the native Rust authentication runtime.
package stargate

/*
#cgo LDFLAGS: -lgo
#include "stargate.h"
#include <stdlib.h>
*/
import "C"
import (
	"context"
	"encoding/json"
	"errors"
	"io"
	"net"
	"net/http"
	"strings"
	"sync"
	"unsafe"
)

type TursoConfig struct {
	Type        string  `json:"type"`
	Path        string  `json:"path"`
	Connections uint32  `json:"connections,omitempty"`
	RetryLimit  *uint32 `json:"retry_limit,omitempty"`
}

func Turso(path string) TursoConfig { return TursoConfig{Type: "turso", Path: path} }

type OIDC struct {
	Name         string `json:"name"`
	Issuer       string `json:"issuer"`
	ClientID     string `json:"client_id"`
	ClientSecret string `json:"client_secret"`
}
type SessionConfig struct {
	TTLSeconds uint32   `json:"ttl_seconds,omitempty"`
	Scopes     []string `json:"scopes,omitempty"`
}
type LocalConfig struct {
	LoginAttempts      uint32  `json:"login_attempts,omitempty"`
	LoginWindowSeconds uint32  `json:"login_window_seconds,omitempty"`
	InitialAdminUserID *string `json:"initial_admin_user_id,omitempty"`
}
type BrandingConfig struct {
	Stylesheet *string `json:"stylesheet,omitempty"`
	AppName    string  `json:"app_name,omitempty"`
	Logo       *string `json:"logo,omitempty"`
	Accent     string  `json:"accent,omitempty"`
}
type Config struct {
	BaseURL               string          `json:"base_url"`
	Storage               TursoConfig     `json:"storage"`
	OIDC                  []OIDC          `json:"oidc,omitempty"`
	Local                 *LocalConfig    `json:"local,omitempty"`
	PathPrefix            string          `json:"path_prefix,omitempty"`
	Session               *SessionConfig  `json:"session,omitempty"`
	Branding              *BrandingConfig `json:"branding,omitempty"`
	MaxBodyBytes          uint32          `json:"max_body_bytes,omitempty"`
	MaxHeaderBytes        uint32          `json:"max_header_bytes,omitempty"`
	TrustedProxies        []string        `json:"trusted_proxies,omitempty"`
	AllowInsecureLoopback bool            `json:"allow_insecure_loopback"`
}
type Identity struct {
	Subject  string         `json:"subject"`
	UserID   *string        `json:"user_id"`
	Email    *string        `json:"email"`
	AuthType string         `json:"auth_type"`
	Scopes   []string       `json:"scopes"`
	Claims   map[string]any `json:"claims"`
}
type Policy struct {
	Type   string   `json:"type"`
	Scopes []string `json:"scopes,omitempty"`
}
type Decision struct {
	Allowed bool `json:"allowed"`
	Status  int  `json:"status"`
}
type Request struct {
	Method  string      `json:"method"`
	Path    string      `json:"path"`
	Query   *string     `json:"query"`
	Headers [][2]string `json:"headers"`
	Body    []byte      `json:"body"`
	PeerIP  *string     `json:"peer_ip"`
}

// MarshalJSON preserves WIT list<u8> instead of Go's default base64 encoding of []byte.
func (r Request) MarshalJSON() ([]byte, error) {
	type alias Request
	values := make([]uint16, len(r.Body))
	for i, b := range r.Body {
		values[i] = uint16(b)
	}
	return json.Marshal(struct {
		alias
		Body []uint16 `json:"body"`
	}{alias: alias(r), Body: values})
}

type Response struct {
	Status  int         `json:"status"`
	Headers [][2]string `json:"headers"`
	Body    []byte      `json:"body"`
}

func (r Response) MarshalJSON() ([]byte, error) {
	type alias Response
	values := make([]uint16, len(r.Body))
	for i, b := range r.Body {
		values[i] = uint16(b)
	}
	return json.Marshal(struct {
		alias
		Body []uint16 `json:"body"`
	}{alias: alias(r), Body: values})
}

type Outcome struct {
	Type            string      `json:"type"`
	Response        *Response   `json:"response,omitempty"`
	Identity        *Identity   `json:"identity,omitempty"`
	ResponseHeaders [][2]string `json:"response_headers,omitempty"`
}
type Auth struct {
	mu     sync.RWMutex
	handle uint64
	prefix string
	limit  int64
}

func native(handle uint64, operation string, input any, output any) error {
	data, err := json.Marshal(input)
	if err != nil {
		return err
	}
	op := C.CString(operation)
	in := C.CString(string(data))
	defer C.free(unsafe.Pointer(op))
	defer C.free(unsafe.Pointer(in))
	result := C.stargate_call(C.uint64_t(handle), op, in)
	if result == nil {
		return errors.New("runtime unavailable")
	}
	defer C.stargate_free(result)
	raw := []byte(C.GoString(result))
	var failure struct {
		Error string `json:"error"`
	}
	if err = json.Unmarshal(raw, &failure); err != nil {
		return err
	}
	if failure.Error != "" {
		return errors.New(failure.Error)
	}
	return json.Unmarshal(raw, output)
}
func New(config Config) (*Auth, error) {
	if C.stargate_abi_version() != 1 {
		return nil, errors.New("unsupported stargate ABI")
	}
	for i := range config.OIDC {
		if config.OIDC[i].Name == "" {
			config.OIDC[i].Name = "default"
		}
	}
	var result struct {
		Handle uint64 `json:"handle"`
	}
	if err := native(0, "create", config, &result); err != nil {
		return nil, err
	}
	prefix := config.PathPrefix
	if prefix == "" {
		prefix = "/auth"
	}
	limit := int64(config.MaxBodyBytes)
	if limit == 0 {
		limit = 65536
	}
	return &Auth{handle: result.Handle, prefix: prefix, limit: limit}, nil
}
func (a *Auth) Close() error {
	a.mu.Lock()
	defer a.mu.Unlock()
	if a.handle == 0 {
		return nil
	}
	if C.stargate_destroy(C.uint64_t(a.handle)) != 0 {
		return errors.New("invalid handle")
	}
	a.handle = 0
	return nil
}
func (a *Auth) Handle(request Request) (Outcome, error) {
	a.mu.RLock()
	defer a.mu.RUnlock()
	var out Outcome
	if request.Headers == nil {
		request.Headers = [][2]string{}
	}
	if request.Body == nil {
		request.Body = []byte{}
	}
	err := native(a.handle, "handle", request, &out)
	return out, err
}
func (a *Auth) Authorize(identity *Identity, policy Policy) (Decision, error) {
	a.mu.RLock()
	defer a.mu.RUnlock()
	if policy.Type == "scopes" && policy.Scopes == nil {
		policy.Scopes = []string{}
	}
	var out Decision
	err := native(a.handle, "authorize", map[string]any{"identity": identity, "policy": policy}, &out)
	return out, err
}

type identityKey struct{}
type runtimeKey struct{}

func IdentityFromContext(ctx context.Context) *Identity {
	identity, _ := ctx.Value(identityKey{}).(*Identity)
	return identity
}
func write(w http.ResponseWriter, response *Response) {
	for _, h := range response.Headers {
		w.Header().Add(h[0], h[1])
	}
	w.WriteHeader(response.Status)
	_, _ = w.Write(response.Body)
}
func (a *Auth) Middleware(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if owner, _ := r.Context().Value(runtimeKey{}).(*Auth); owner == a {
			next.ServeHTTP(w, r)
			return
		}
		path := r.URL.EscapedPath()
		owned := path == a.prefix || strings.HasPrefix(path, a.prefix+"/")
		body := []byte{}
		if owned {
			data, err := io.ReadAll(io.LimitReader(r.Body, a.limit+1))
			if err != nil {
				http.Error(w, "invalid request", 400)
				return
			}
			if int64(len(data)) > a.limit {
				http.Error(w, "request too large", 413)
				return
			}
			body = data
		}
		headers := [][2]string{}
		for key, values := range r.Header {
			for _, value := range values {
				headers = append(headers, [2]string{key, value})
			}
		}
		if r.Host != "" {
			headers = append(headers, [2]string{"host", r.Host})
		}
		var query *string
		if r.URL.RawQuery != "" {
			query = &r.URL.RawQuery
		}
		var peer *string
		if host, _, err := net.SplitHostPort(r.RemoteAddr); err == nil {
			peer = &host
		}
		outcome, err := a.Handle(Request{Method: r.Method, Path: path, Query: query, Headers: headers, Body: body, PeerIP: peer})
		if err != nil {
			http.Error(w, "runtime unavailable", 503)
			return
		}
		if outcome.Type == "respond" {
			write(w, outcome.Response)
			return
		}
		for _, h := range outcome.ResponseHeaders {
			w.Header().Add(h[0], h[1])
		}
		next.ServeHTTP(w, r.WithContext(context.WithValue(context.WithValue(r.Context(), identityKey{}, outcome.Identity), runtimeKey{}, a)))
	})
}
func (a *Auth) Handler() http.Handler                  { return a.Middleware(http.NotFoundHandler()) }
func (a *Auth) Require(next http.Handler) http.Handler { return a.RequireScopes(next) }
func (a *Auth) RequireScopes(next http.Handler, scopes ...string) http.Handler {
	return a.Middleware(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		policy := Policy{Type: "authenticated"}
		if len(scopes) > 0 {
			policy = Policy{Type: "scopes", Scopes: scopes}
		}
		decision, err := a.Authorize(IdentityFromContext(r.Context()), policy)
		if err != nil {
			http.Error(w, "runtime unavailable", 503)
			return
		}
		if !decision.Allowed {
			http.Error(w, "access denied", decision.Status)
			return
		}
		next.ServeHTTP(w, r)
	}))
}
