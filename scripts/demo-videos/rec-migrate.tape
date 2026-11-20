Output raw/migrate-raw.mp4
Set Shell "bash"
Set FontSize 22
Set Width 1920
Set Height 1080
Set Padding 30
Set Theme "Dracula"
Set TypingSpeed 40ms

Type "ssh sus@212.8.248.187" Enter
Sleep 3s
Type "cd ~/gk-demo && clear" Enter
Sleep 1s
Type "guestkit migrate-plan ubuntu-demo.qcow2 --target kvm" Enter
Sleep 25s
Type "exit" Enter
Sleep 500ms
