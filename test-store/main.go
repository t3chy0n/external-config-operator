package main

import (
	"encoding/json"
	"net/http"
	"log"
)

type Response struct {
	Message string `json:"message"`
	Status  int    `json:"status"`
}

func jsonHandler(w http.ResponseWriter, r *http.Request) {
	response := Response{
		Message: "Hello, this is a sample JSON response!",
		Status:  200,
	}
	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(response)
}

func main() {
	http.HandleFunc("/json", jsonHandler)
	log.Println("Server started on :8080")
	log.Fatal(http.ListenAndServe(":8080", nil))
}