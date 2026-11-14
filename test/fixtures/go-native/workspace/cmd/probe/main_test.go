package main

import "testing"

func TestMessage(t *testing.T) {
	if message == "" {
		t.Fatal("empty embedded message")
	}
}
