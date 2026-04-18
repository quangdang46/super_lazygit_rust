package tail

import (
	"fmt"
	"log"
	"os"

	"github.com/aybabtme/humanlog"
)

// TailLogs lets us run `slg --logs` to print the logs produced by other slg processes.
// This makes for easier debugging.
func TailLogs(logFilePath string) {
	fmt.Printf("Tailing log file %s\n\n", logFilePath)

	opts := humanlog.DefaultOptions
	opts.Truncates = false

	_, err := os.Stat(logFilePath)
	if err != nil {
		if os.IsNotExist(err) {
			log.Fatal("Log file does not exist. Run `slg --debug` first to create the log file")
		}
		log.Fatal(err)
	}

	tailLogsForPlatform(logFilePath, opts)
}
