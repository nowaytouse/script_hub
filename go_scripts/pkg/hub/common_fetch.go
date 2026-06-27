package hub

import (
	"fmt"
	"io"
	"net/http"
	"time"
)

func CurlFetch(urlStr string, ua string) []byte {
	client := &http.Client{Timeout: 60 * time.Second}
	req, err := http.NewRequest("GET", urlStr, nil)
	if err != nil {
		return nil
	}
	if ua != "" {
		req.Header.Set("User-Agent", ua)
	}

	for i := 0; i < 2; i++ {
		resp, err := client.Do(req)
		if err != nil {
			time.Sleep(2 * time.Second)
			continue
		}
		defer resp.Body.Close()
		if resp.StatusCode != http.StatusOK {
			time.Sleep(2 * time.Second)
			continue
		}
		data, err := io.ReadAll(resp.Body)
		if err == nil {
			return data
		}
	}
	Warn(fmt.Sprintf("Failed to fetch %s", urlStr))
	return nil
}
