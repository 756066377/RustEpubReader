BEGIN {
    heading = "## [" tag "]"
}

index($0, heading) == 1 {
    found = 1
    next
}

found && /^## \[/ {
    exit
}

found {
    print
}

END {
    if (!found) {
        exit 1
    }
}
