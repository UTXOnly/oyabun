package main

import (
	"log"
	"net/http"
	"os"

	"github.com/oyabaun/oyabaun/relay/internal/relay"
)

func main() {
	addr := ":8765"
	if v := os.Getenv("OYABAUN_RELAY_ADDR"); v != "" {
		addr = v
	}
	var gameLog *relay.EventLog
	if p := os.Getenv("OYABAUN_GAME_LOG"); p != "" {
		gl, err := relay.OpenEventLog(p)
		if err != nil {
			log.Printf("OYABAUN_GAME_LOG %q: %v (gameplay events not persisted)", p, err)
		} else {
			gameLog = gl
			log.Printf("gameplay log: %s", p)
		}
	}
	h := relay.NewHub(gameLog)
	log.Printf("relay event pubkey (snap/join ack): %s", h.RelayPublicKeyHex())
	mux := http.NewServeMux()
	mux.HandleFunc("/ws", h.HandleWS)
	mux.HandleFunc("/healthz", func(w http.ResponseWriter, _ *http.Request) {
		w.WriteHeader(http.StatusOK)
		_, _ = w.Write([]byte("ok"))
	})
	log.Printf("oyabaun-relay listening %s", addr)
	log.Fatal(http.ListenAndServe(addr, mux))
}
