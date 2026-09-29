SIDE=host
ARGS=(patch 99999)
IGNORE=(exit)  # bash exits 0 after a failed patch (its loop's break); the port exits 1
