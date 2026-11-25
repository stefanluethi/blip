# sonde

Configure CMake with `-DCMAKE_BUILD_TYPE=Release`, never without a build type:

```bash
cmake -S sonde -B sonde/build -DCMAKE_BUILD_TYPE=Release
cmake --build sonde/build --parallel
ctest --test-dir sonde/build --output-on-failure
```

Cross builds for Cortex-M use `MinSizeRel`, as in `.github/workflows/sonde.yml`.
