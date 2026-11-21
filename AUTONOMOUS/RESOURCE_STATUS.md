# Resource Status

Machine: 4 logical CPUs, 7.7 GiB RAM, 4 GiB swap.
Observed during organisation run: load average 14-17 (heavily loaded by multiple concurrent OpenCode2 sessions), ~3.6 GiB RAM used, 2.4 GiB swap used.
Decision: no additional subagents spawned; all orchestrators executed work inline because runtime depth limit is 1.
Avoid further parallel heavy tasks until load drops.
