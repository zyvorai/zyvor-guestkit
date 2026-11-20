# GuestKit cutover gate (subset evaluated in-process; full OPA optional).
package guestkit

deny[msg] {
  input.hard_blocked == true
  msg := "passport hard-blocked"
}

deny[msg] {
  input.scores.boot < 80
  msg := "boot score below 80"
}

deny[msg] {
  input.scores.migration < 80
  msg := "migration score below 80"
}
