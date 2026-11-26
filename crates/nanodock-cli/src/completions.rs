pub fn cmd_completions(shell: Option<&str>) -> anyhow::Result<u8> {
    let Some(target) = shell else {
        anyhow::bail!("usage: nanodock completions <powershell|bash|zsh>");
    };

    let script = generate_completion_script(target)?;
    print!("{script}");
    Ok(0)
}

pub fn generate_completion_script(shell: &str) -> anyhow::Result<&'static str> {
    match shell.to_ascii_lowercase().as_str() {
        "powershell" | "pwsh" => Ok(POWERSHELL_COMPLETION),
        "bash" => Ok(BASH_COMPLETION),
        "zsh" => Ok(ZSH_COMPLETION),
        other => anyhow::bail!("unsupported shell '{other}'. Expected powershell, bash, or zsh."),
    }
}

const POWERSHELL_COMPLETION: &str = r#"# NanoDock PowerShell completion
Register-ArgumentCompleter -Native -CommandName nanodock -ScriptBlock {
    param($wordToComplete, $commandAst, $cursorPosition)
    $commands = @('start', 'stop', 'status', 'trim', 'config', 'logs', 'doctor', 'completions', 'version', 'help', 'ps', 'run', 'images', 'compose', 'build', 'exec', 'logs', 'volume', 'network')
    $commands | Where-Object { $_ -like "$wordToComplete*" } | ForEach-Object {
        [System.Management.Automation.CompletionResult]::new($_, $_, 'ParameterValue', $_)
    }
}
"#;

const BASH_COMPLETION: &str = r#"# NanoDock bash completion
_nanodock_completions() {
    local cur="${COMP_WORDS[COMP_CWORD]}"
    local cmds="start stop status trim config logs doctor completions version help ps run images compose build exec volume network"
    COMPREPLY=( $(compgen -W "${cmds}" -- "${cur}") )
}
complete -F _nanodock_completions nanodock
"#;

const ZSH_COMPLETION: &str = r#"#compdef nanodock
_nanodock() {
    local -a commands
    commands=(
        'start:Start core-daemon and wake the WSL2 engine'
        'stop:Gracefully hibernate the WSL2 engine'
        'status:Show engine state and active streams'
        'trim:Reclaim unused WSL2 virtual disk space'
        'config:Print active engine configuration'
        'logs:Show core-daemon and dockerd logs'
        'doctor:Run NanoDock environment diagnostics'
        'completions:Generate shell completion scripts'
        'version:Print NanoDock CLI version'
        'help:Show help message'
    )
    _describe 'command' commands
}
compdef _nanodock nanodock
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_completions_for_supported_shells() {
        assert!(generate_completion_script("powershell")
            .unwrap()
            .contains("Register-ArgumentCompleter"));
        assert!(generate_completion_script("bash")
            .unwrap()
            .contains("complete -F"));
        assert!(generate_completion_script("zsh")
            .unwrap()
            .contains("#compdef nanodock"));
        assert!(generate_completion_script("fish").is_err());
    }
}
