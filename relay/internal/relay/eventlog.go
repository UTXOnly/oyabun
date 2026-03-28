package relay

import (
	"encoding/json"
	"os"
	"sync"
	"time"
)

type EventLog struct {
	mu sync.Mutex
	f  *os.File
}

func OpenEventLog(path string) (*EventLog, error) {
	f, err := os.OpenFile(path, os.O_CREATE|os.O_WRONLY|os.O_APPEND, 0644)
	if err != nil {
		return nil, err
	}
	return &EventLog{f: f}, nil
}

func (e *EventLog) Append(m map[string]any) {
	if e == nil || e.f == nil {
		return
	}
	m["ts_ms"] = time.Now().UnixMilli()
	b, err := json.Marshal(m)
	if err != nil {
		return
	}
	e.mu.Lock()
	defer e.mu.Unlock()
	_, _ = e.f.Write(append(b, '\n'))
	_ = e.f.Sync()
}
