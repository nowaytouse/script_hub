package main
import (
	"fmt"
	"regexp"
)
func main() {
	content := "script-path=https://raw.githubusercontent.com/app2smile/rules/master/js/tieba-json.js"
	regex := regexp.MustCompile("(?i)https://raw\\.githubusercontent\\.com/([^/]+)/([^/]+)/([^/]+)/([^\"'\\s]+\\.[a-zA-Z0-9]+)")
	repl := "https://cdn.jsdelivr.net/gh/$1/$2@$3/$4"
	fmt.Println(regex.ReplaceAllString(content, repl))
}
