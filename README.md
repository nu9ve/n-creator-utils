# n-creator-utils

nu9ve python and shell based scripts to help on a creative workflow


### dependencies

```
python 3.9
ffmpeg
```


## getting started

- clone project
    `git clone git@github.com:nu9ve/n-creator-utils.git`
- install dependencies
    `brew install ffmpeg`
- configure path
    include the following line in your shell config (~/.profile, ~/.zshrc, etc)
    ```shell
    export alias nu9ve='python3 $HOME/path/to/n-creator-utils/nu9ve.py'
    ```
    *don't forget to update the path to this project's path
- create


### run

after configuring your path you should be able to run `nu9ve` from your terminal, you can then select from the following subcommands:

#### save
```console
nu9ve save [dir_path|all] [general_tag] [specific_tag]
```

#### [clip](https://github.com/nu9ve/n-creator-utils/tree/master/clipper)
```console
nu9ve clip [videopath]
```

<!-- 
end with an example of getting some data out of the system or using it for a little demo
## running the tests (xd)

filemanager should output correct actions
clipper should check ffmpeg version or export is working

```
nu9ve test [function]
```

## contributing

please read [CONTRIBUTING.md](https://gist.github.com/PurpleBooth/b24679402957c63ec426) for details on our code of conduct, and the process for submitting pull requests to us.

## license

this project is licensed under the MIT License - see the [LICENSE.md](LICENSE.md) file for details

## acknowledgments

* hat tip to anyone whose code was used
* thanks for the [template](https://gist.github.com/PurpleBooth/b24679402957c63ec426) purplebooth -->
