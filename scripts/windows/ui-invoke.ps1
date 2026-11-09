# origen: nove scripts/windows/ui-invoke.ps1 (sin cambios de comportamiento).
# QA helper: invoke (click) the Nth element with a given accessible name in a
# process window through UI Automation, the same tree Narrator uses.
# Usage: ui-invoke.ps1 -ProcessId 1234 -Name "Fuentes" [-Index 0] [-List]
param(
    [Parameter(Mandatory = $true)][int]$ProcessId,
    [string]$Name = '', [int]$Index = 0, [switch]$List, [string]$SetValue = $null
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
$A = [System.Windows.Automation.AutomationElement]
$cond = New-Object System.Windows.Automation.PropertyCondition ($A::ProcessIdProperty), $ProcessId
$deadline = (Get-Date).AddSeconds(10)
do { $windows = $A::RootElement.FindAll([System.Windows.Automation.TreeScope]::Children, $cond); if ($windows.Count -eq 0) { Start-Sleep -Milliseconds 200 } }
while ($windows.Count -eq 0 -and (Get-Date) -lt $deadline)
if ($windows.Count -eq 0) { throw "No window for PID $ProcessId" }
# A process can own several top-level windows, and FindAll stops at the
# cross-process WebView2 fragment; walk node by node and skip failing subtrees.
$walker = [System.Windows.Automation.TreeWalker]::ControlViewWalker
$all = New-Object System.Collections.Generic.List[System.Windows.Automation.AutomationElement]
function Visit($node, [int]$depth) {
    if ($depth -gt 60) { return }
    try { $child = $walker.GetFirstChild($node) } catch { return }
    while ($child) {
        $all.Add($child)
        Visit $child ($depth + 1)
        try { $child = $walker.GetNextSibling($child) } catch { $child = $null }
    }
}
foreach ($window in $windows) { Visit $window 0 }
if ($List) {
    foreach ($e in $all) { $n = $e.Current.Name; if ($n) { "{0} | {1}" -f $e.Current.ControlType.ProgrammaticName, $n } }
    return
}
$matches = @($all | Where-Object { $_.Current.Name -eq $Name })
# Prefer actionable elements (a button's inner text has the same name).
$actionable = @($matches | Where-Object {
    $t = $_.Current.ControlType
    $t -ne [System.Windows.Automation.ControlType]::Text -and $t -ne [System.Windows.Automation.ControlType]::Group
})
if ($actionable.Count -gt 0) { $matches = $actionable }
if ($matches.Count -le $Index) { throw "Element '$Name' #$Index not found ($($matches.Count) matches)" }
$el = $matches[$Index]
if ($PSBoundParameters.ContainsKey('SetValue')) {
    $vp = $null
    if (-not $el.TryGetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern, [ref]$vp)) { throw "Element '$Name' has no ValuePattern" }
    $vp.SetValue($SetValue); "set '$Name'"; return
}
foreach ($pattern in 'InvokePatternIdentifiers', 'SelectionItemPatternIdentifiers', 'TogglePatternIdentifiers') {
    $id = ([type]"System.Windows.Automation.$pattern")::Pattern
    $p = $null
    if ($el.TryGetCurrentPattern($id, [ref]$p)) {
        switch ($pattern) {
            'InvokePatternIdentifiers' { $p.Invoke() }
            'SelectionItemPatternIdentifiers' { $p.Select() }
            'TogglePatternIdentifiers' { $p.Toggle() }
        }
        "invoked '$Name' via $pattern"; return
    }
}
throw "Element '$Name' supports no invoke/select/toggle pattern"
