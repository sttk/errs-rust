#!/usr/bin/env bash

errcheck() {
  exitcd=$1
  if [[ "$exitcd" != "0" ]]; then
    exit $exitcd
  fi
}

clean() {
  cargo clean
  errcheck $?
}

format() {
  cargo fmt
  errcheck $?
}

compile() {
  cargo build --all-features 
  errcheck $?
}

test() {
  cargo test --all-features -- --show-output
  errcheck $?
}

unit() {
  cargo test --all-features -- --show-output $1
  errcheck $?
}

cover() {
  cargo llvm-cov clean
  errcheck $?
  cargo llvm-cov --all-features --html --quiet
  errcheck $?
  cargo llvm-cov report
  errcheck $?
}

bench() {
  cargo +nightly bench --quiet -- $1
  errcheck $?
}

doc() {
  cargo +nightly rustdoc --all-features -- --cfg docsrs
  errcheck $?
}

msrv() {
  cargo msrv find --ignore-lockfile --no-check-feedback
  errcheck $?
}

if [[ "$#" == "0" ]]; then
  #clean
  format
  compile
  test
  doc
  cover

elif [[ "$1" == "unit" ]]; then
  unit $2

else
  for a in "$@"; do
    case "$a" in
    clean)
      clean
      ;;
    format)
      format
      ;;
    compile)
      compile
      ;;
    test)
      test
      ;;
    doc)
      doc
      ;;
    cover)
      cover
      ;;
    bench)
      bench
      ;;
    msrv)
      msrv
      ;;
    *)
      echo "Bad task: $a"
      exit 1
      ;;
    esac
  done
fi
