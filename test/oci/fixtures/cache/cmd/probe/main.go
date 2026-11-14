package main
import ("fmt"; "os"; "strings")
const version = "v1"
func main() {
    content, err := os.ReadFile("/app/message.txt")
    if err != nil { panic(err) }
    directory, err := os.Getwd()
    if err != nil { panic(err) }
    fmt.Printf("%s %s %s %d %s\n", version, os.Getenv("OCI_PROBE"), strings.TrimSpace(string(content)), os.Getuid(), directory)
}
