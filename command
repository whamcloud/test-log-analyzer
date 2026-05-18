sudo dtrace -n 'profile-997 /execname == "ddnn"/ { @[ustack(10)] = count(); }' -o out.pretty_profile -c './target/profiling/ddnn' | awk '
  # First pass: Store lines and calculate the total sum of all samples
  { lines[NR] = $0; if ($NF ~ /^[0-9]+$/) total += $NF } 
  
  # Second pass: Print lines in reverse order (highest CPU first) with percentage
  END {
    for (i = NR; i > 0; i--) {
      split(lines[i], parts);
      count = parts[length(parts)];
      if (count ~ /^[0-9]+$/ && total > 0) {
        pct = (count / total) * 100;
        printf "%s (%.2f%%)\n", lines[i], pct;
      } else {
        print lines[i];
      }
    }
  }
'

sudo dtrace -n '
#define HIGHEST_REVISION_FIRST 1

/* 1. Count every time a sample is taken across the whole system */
profile-997 /execname == "ddnn"/ {
    @counts[ustack(100)] = count();
    @total = count();
}

/* 2. When the binary finishes, calculate and print the percentages */
dtrace:::END {
    printa("Stack:\n%k\nPercent: %% \nCount: %@d\n\n", @counts);
}
' -c './target/profiling/ddnn'
