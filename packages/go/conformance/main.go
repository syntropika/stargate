package main

import (
	"bufio"
	"encoding/json"
	"fmt"
	"os"
	stargate "github.com/syntropika/stargate/packages/go"
)

func main() {
	scanner := bufio.NewScanner(os.Stdin)
	scanner.Buffer(make([]byte, 65536), 2*1024*1024)
	var auth *stargate.Auth
	defer func() {
		if auth != nil {
			_ = auth.Close()
		}
	}()
	for scanner.Scan() {
		var command struct {
			Operation string          `json:"operation"`
			Input     json.RawMessage `json:"input"`
		}
		var out any
		err := json.Unmarshal(scanner.Bytes(), &command)
		if err == nil {
			switch command.Operation {
			case "create":
				if auth != nil {
					_ = auth.Close()
				}
				var config stargate.Config
				err = json.Unmarshal(command.Input, &config)
				if err == nil {
					auth, err = stargate.New(config)
					out = map[string]bool{"ready": true}
				}
			case "handle":
				var request stargate.Request
				err = json.Unmarshal(command.Input, &request)
				if err == nil {
					out, err = auth.Handle(request)
				}
			case "authorize":
				var input struct {
					Identity *stargate.Identity `json:"identity"`
					Policy   stargate.Policy    `json:"policy"`
				}
				err = json.Unmarshal(command.Input, &input)
				if err == nil {
					out, err = auth.Authorize(input.Identity, input.Policy)
				}
			default:
				out = map[string]string{"error": "invalid operation"}
			}
		}
		if err != nil {
			out = map[string]string{"error": "native call failed"}
		}
		data, _ := json.Marshal(out)
		fmt.Println(string(data))
	}
}
